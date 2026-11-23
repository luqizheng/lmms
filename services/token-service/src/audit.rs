//! Persistent audit ledger for JWT signing-key lifecycle operations.
//!
//! Spec: `docs/superpowers/specs/2026-11-23-token-jwt-key-operations-hardening-design.md`.
//!
//! The ledger is an append-only JSON-lines file (`keystore-audit.jsonl`) that
//! lives next to `keystore.json` (0600, `fsync` per record). Each line uses the
//! unified `audit_events` row shape (`event_id/event_type/occurred_at/trace_id/
//! producer/payload`) so records can be shipped to audit-service later without
//! a schema change.
//!
//! Why a local file instead of the MQ/outbox audit pipeline: the operations
//! that most need auditing (backup/restore) are performed by the offline
//! `keytool`, which has no DB/MQ access, and keystore state is a file so no DB
//! transaction can be atomic with it. See the spec for the full rationale.
//!
//! Hard rule: this module has **no field capable of carrying key material**.
//! Records hold actor/action/kid/result/detail only; tests assert the ledger
//! never contains PEM markers.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

/// File name of the audit ledger, resolved next to the keystore file.
pub const AUDIT_LEDGER_FILENAME: &str = "keystore-audit.jsonl";
/// Producer identity written into every record.
pub const PRODUCER: &str = "token-service";

/// Key-lifecycle operation being audited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    Create,
    Import,
    Rotate,
    Retire,
    Delete,
    Backup,
    Restore,
}

impl AuditAction {
    pub fn as_str(self) -> &'static str {
        match self {
            AuditAction::Create => "create",
            AuditAction::Import => "import",
            AuditAction::Rotate => "rotate",
            AuditAction::Retire => "retire",
            AuditAction::Delete => "delete",
            AuditAction::Backup => "backup",
            AuditAction::Restore => "restore",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditResult {
    Success,
    Failed,
}

impl AuditResult {
    pub fn as_str(self) -> &'static str {
        match self {
            AuditResult::Success => "success",
            AuditResult::Failed => "failed",
        }
    }
}

/// Who performed the operation and, when available, the request trace id.
#[derive(Debug, Clone)]
pub struct AuditContext {
    /// JWT `sub` of the caller (admin API) or `keytool` / `system`.
    pub actor: String,
    /// Correlation id from the active tracing span, when present.
    pub trace_id: Option<String>,
}

impl AuditContext {
    pub fn new(actor: impl Into<String>, trace_id: Option<String>) -> Self {
        Self {
            actor: actor.into(),
            trace_id,
        }
    }

    /// Startup-side operations (first-boot create / legacy import).
    pub fn system() -> Self {
        Self::new("system", None)
    }

    /// Offline `keytool` operations.
    pub fn cli() -> Self {
        Self::new("keytool", None)
    }
}

/// Non-secret payload: nothing here can reconstruct a private key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditPayload {
    pub actor: String,
    pub action: String,
    pub result: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub related_kid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// One audit line, shaped like a row of audit-service's `audit_events`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub event_id: String,
    pub event_type: String,
    pub occurred_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    pub producer: String,
    pub payload: AuditPayload,
}

/// Append-only ledger handle (cheap to clone; the path is the only state).
#[derive(Debug, Clone)]
pub struct AuditLedger {
    path: PathBuf,
}

impl AuditLedger {
    /// Ledger that lives next to `keystore_path`.
    pub fn beside_keystore(keystore_path: &Path) -> Self {
        let dir = keystore_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            path: dir.join(AUDIT_LEDGER_FILENAME),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one durable audit record. `Err` means the record was NOT
    /// persisted; callers decide whether to log (see [`Self::record`]).
    pub fn append(
        &self,
        ctx: &AuditContext,
        action: AuditAction,
        result: AuditResult,
        kid: Option<&str>,
        related_kid: Option<&str>,
        detail: Option<&str>,
    ) -> Result<AuditRecord, String> {
        let record = AuditRecord {
            event_id: uuid::Uuid::new_v4().to_string(),
            event_type: event_type(action, result),
            occurred_at: chrono::Utc::now().timestamp(),
            trace_id: ctx.trace_id.clone(),
            producer: PRODUCER.to_string(),
            payload: AuditPayload {
                actor: ctx.actor.clone(),
                action: action.as_str().to_string(),
                result: result.as_str().to_string(),
                kid: kid.map(str::to_string),
                related_kid: related_kid.map(str::to_string),
                detail: detail.map(str::to_string),
            },
        };
        let mut line =
            serde_json::to_string(&record).map_err(|e| format!("serialize audit record: {e}"))?;
        line.push('\n');

        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("create audit dir {}: {e}", parent.display()))?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ =
                        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
                }
            }
        }
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)
            .map_err(|e| format!("open audit ledger {}: {e}", self.path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("chmod audit ledger: {e}"))?;
        }
        f.write_all(line.as_bytes())
            .map_err(|e| format!("write audit ledger: {e}"))?;
        f.sync_all()
            .map_err(|e| format!("fsync audit ledger: {e}"))?;
        Ok(record)
    }

    /// Best-effort append used by key operations: a write failure must never
    /// roll back an already-persisted key change, but it must never be
    /// silent either (ERROR log + failure counter).
    pub fn record(
        &self,
        ctx: &AuditContext,
        action: AuditAction,
        result: AuditResult,
        kid: Option<&str>,
        related_kid: Option<&str>,
        detail: Option<&str>,
    ) {
        if let Err(e) = self.append(ctx, action, result, kid, related_kid, detail) {
            audit_failure_counter().inc();
            tracing::error!(
                error = %e,
                audit_action = action.as_str(),
                audit_result = result.as_str(),
                kid = kid.unwrap_or(""),
                "FAILED to persist keystore audit record; key operation already applied"
            );
        }
    }

    /// Read every record (oldest first). Used by `keytool audit` and tests.
    pub fn read_all(&self) -> Result<Vec<AuditRecord>, String> {
        let raw = match std::fs::read_to_string(&self.path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("read audit ledger {}: {e}", self.path.display())),
        };
        raw.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                serde_json::from_str::<AuditRecord>(l)
                    .map_err(|e| format!("parse audit record: {e}"))
            })
            .collect()
    }
}

fn event_type(action: AuditAction, result: AuditResult) -> String {
    match (action, result) {
        (AuditAction::Restore, AuditResult::Failed) => "token.jwt_key.restore_failed".to_string(),
        _ => format!("token.jwt_key.{}", action.as_str()),
    }
}

/// `token_keystore_audit_write_failures_total` — alert on any non-zero value:
/// it means a key-lifecycle action happened without a persisted audit record.
fn audit_failure_counter() -> &'static prometheus::IntCounter {
    use std::sync::OnceLock;
    static COUNTER: OnceLock<prometheus::IntCounter> = OnceLock::new();
    COUNTER.get_or_init(|| {
        let opts = prometheus::Opts::new(
            "token_keystore_audit_write_failures_total",
            "Keystore audit ledger append failures (key operation applied but not audited)",
        );
        match prometheus::IntCounter::with_opts(opts) {
            Ok(c) => {
                let _ = prometheus::register(Box::new(c.clone()));
                c
            }
            Err(_) => prometheus::IntCounter::new(
                "token_keystore_audit_write_failures_total",
                "Keystore audit ledger append failures",
            )
            .expect("static counter opts are valid"),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_are_append_only_and_parseable() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = AuditLedger::beside_keystore(&dir.path().join("keystore.json"));
        let ctx = AuditContext::new("iam-service", Some("trace-1".into()));
        ledger
            .append(
                &ctx,
                AuditAction::Rotate,
                AuditResult::Success,
                Some("k1"),
                Some("k0"),
                None,
            )
            .unwrap();
        ledger
            .append(
                &ctx,
                AuditAction::Retire,
                AuditResult::Failed,
                Some("k0"),
                None,
                Some("not retiring"),
            )
            .unwrap();

        let records = ledger.read_all().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].event_type, "token.jwt_key.rotate");
        assert_eq!(records[0].producer, PRODUCER);
        assert_eq!(records[0].payload.actor, "iam-service");
        assert_eq!(records[0].payload.related_kid.as_deref(), Some("k0"));
        assert_eq!(records[0].trace_id.as_deref(), Some("trace-1"));
        assert_eq!(records[1].event_type, "token.jwt_key.retire");
        assert_eq!(records[1].payload.result, "failed");
        assert_ne!(records[0].event_id, records[1].event_id);
    }

    #[test]
    fn failed_restore_uses_dedicated_event_type() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = AuditLedger::beside_keystore(&dir.path().join("keystore.json"));
        ledger.record(
            &AuditContext::cli(),
            AuditAction::Restore,
            AuditResult::Failed,
            None,
            None,
            Some("checksum mismatch"),
        );
        let records = ledger.read_all().unwrap();
        assert_eq!(records[0].event_type, "token.jwt_key.restore_failed");
        assert_eq!(records[0].payload.action, "restore");
    }
}
