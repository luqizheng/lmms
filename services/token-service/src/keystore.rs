//! JWT signing key keystore: persistence, rotation, backup & restore.
//!
//! Spec: `docs/superpowers/specs/2026-11-23-token-jwt-key-persistence-design.md`.
//!
//! One source of truth on disk: `keystore.json` (default: `keystore.json`
//! next to `TOKEN_JWT_PRIVATE_KEY_PATH`, i.e. `/app/keys/keystore.json` in
//! containers when a volume is mounted there).
//!
//! Startup semantics (`init`) are **fail-closed**: an existing-but-corrupt
//! keystore (bad JSON, unparseable PEM, private/public mismatch, permissive
//! unix permissions) aborts startup instead of silently generating a
//! replacement key — that silent path is exactly what used to invalidate
//! every previously issued JWT on container recreation.
//!
//! Security invariants:
//! - private key material never appears in log output (kid/status only);
//! - writes are atomic (tmp file + fsync + rename) and 0600 (unix);
//! - the keystore directory is 0700 (unix);
//! - removed keys must be `retired` and past `min_retention_sec`.
//!
//! Operational hardening (2026-11-23 hardening spec):
//! - every lifecycle mutation appends a non-secret record to the durable
//!   audit ledger (`keystore-audit.jsonl`, see [`crate::audit`]);
//! - `save` refuses to overwrite a file that changed on disk since it was
//!   loaded ([`KeyStoreError::Stale`]) and serialises the write through a
//!   cross-process lock file (`KeyStoreError::LockBusy`) — a stale write can
//!   never silently delete a key rotated by another process;
//! - backups are wrapped in a versioned envelope with a SHA-256 integrity
//!   checksum; restore validates everything, snapshots the live keystore,
//!   writes under the lock and verifies the result from disk (rolling back
//!   on failure).

use crate::audit::{AuditAction, AuditContext, AuditLedger, AuditResult};
use jwt_simple::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use thiserror::Error;

/// kid assigned to keys imported from the legacy `private.pem`/`public.pem`
/// pair. Historical tokens carry `kid:"default"` in their header (the old
/// hard-coded `JWT_KEY_ID`), so imported keys keep that kid and stay in the
/// JWKS until retired — otherwise every pre-migration token would fail
/// verification with "unknown kid".
pub const LEGACY_KID: &str = "default";
/// On-disk format version. Bumping requires a migration path.
pub const KEYSTORE_VERSION: u32 = 1;
/// Backup envelope format version (independent of `KEYSTORE_VERSION`).
pub const BACKUP_FORMAT_VERSION: u32 = 1;
/// How long a mutation waits for the cross-process write lock before giving
/// up with [`KeyStoreError::LockBusy`].
const LOCK_WAIT: Duration = Duration::from_secs(5);
/// A lock file older than this is considered abandoned (crashed process) and
/// reclaimed. Key operations take milliseconds; 10 min is only a safety net.
const LOCK_STALE_AFTER: Duration = Duration::from_secs(600);

#[derive(Error, Debug)]
pub enum KeyStoreError {
    #[error("keystore load failed (refusing to start, NOT regenerating keys): {0}")]
    Load(String),
    #[error("keystore save failed: {0}")]
    Save(String),
    #[error("keystore file permissions unsafe: {0}")]
    UnsafePermissions(String),
    #[error("invalid keystore operation: {0}")]
    InvalidOperation(String),
    #[error("key generation failed: {0}")]
    Generate(String),
    /// The file on disk changed since this snapshot was loaded (concurrent
    /// `keytool` run or another instance). The write is refused so a stale
    /// snapshot can never overwrite newer key state.
    #[error("keystore changed on disk since it was loaded: {0}")]
    Stale(String),
    /// Another key operation holds the cross-process lock.
    #[error("another key operation is in progress: {0}")]
    LockBusy(String),
    /// Backup checksum/format verification failed.
    #[error("backup integrity check failed: {0}")]
    BackupIntegrity(String),
    /// Backup envelope version is not supported by this build.
    #[error("unsupported backup format: {0}")]
    BackupVersion(String),
    /// Restore wrote the keystore but the post-write verification failed;
    /// the previous state was rolled back.
    #[error("restore failed and the previous keystore was restored: {0}")]
    RestoreFailed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyStatus {
    /// Signs newly issued tokens; published in JWKS.
    Active,
    /// Verify-only (post-rotation); still published in JWKS.
    Retiring,
    /// Verify-only, NOT published in JWKS; deletable after retention.
    Retired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyEntry {
    pub kid: String,
    pub status: KeyStatus,
    /// Unix seconds when this key was created.
    pub created_at: i64,
    /// Unix seconds when the key entered `retired` status.
    #[serde(default)]
    pub retired_at: Option<i64>,
    pub private_pem: String,
    pub public_pem: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyStoreData {
    pub version: u32,
    pub active_kid: String,
    pub keys: Vec<KeyEntry>,
}

/// Versioned, checksummed backup wrapper. `keystore_sha256` is the SHA-256
/// (hex) of the compact JSON serialization of `keystore`, so truncation or
/// content edits are detected before anything is written to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupEnvelope {
    pub backup_format_version: u32,
    pub created_at: i64,
    pub producer: String,
    pub keystore_sha256: String,
    pub keystore: KeyStoreData,
}

#[derive(Debug)]
pub struct KeyStore {
    path: PathBuf,
    min_retention_sec: i64,
    data: KeyStoreData,
    audit: AuditLedger,
    /// SHA-256 of the on-disk keystore bytes as of the last load/save;
    /// `None` while the file does not exist (first-boot path).
    fingerprint: Option<String>,
}

impl KeyStore {
    // ─────────────────────────────────────────────────────────────────────
    // Startup entry points
    // ─────────────────────────────────────────────────────────────────────

    /// Startup resolution order (see module docs for the fail-closed matrix):
    /// 1. keystore.json exists → `load` (any corruption = hard error);
    /// 2. legacy private+public PEM pair exists → import (kid stays
    ///    `"default"` for backward compatibility), atomic save, then serve;
    /// 3. neither exists → generate a fresh key pair, atomic save;
    /// 4. exactly one legacy PEM present (or unreadable IO error) → hard
    ///    error; never overwrite half of an existing key set.
    pub fn init(
        keystore_path: &Path,
        legacy_private_path: &str,
        legacy_public_path: &str,
        min_retention_sec: i64,
    ) -> Result<Self, KeyStoreError> {
        if keystore_path.exists() {
            return Self::load(keystore_path, min_retention_sec);
        }
        match (
            read_opt(legacy_private_path)?,
            read_opt(legacy_public_path)?,
        ) {
            (Some(priv_pem), Some(pub_pem)) => {
                // Import: parse & cross-check BEFORE touching the disk so a
                // corrupt legacy PEM is a fail-closed error, not an overwrite.
                let mut ks =
                    Self::from_legacy_pems(priv_pem, pub_pem, keystore_path, min_retention_sec)?;
                ks.save()?;
                ks.audit.record(
                    &AuditContext::system(),
                    AuditAction::Import,
                    AuditResult::Success,
                    Some(&ks.data.active_kid),
                    None,
                    Some("imported legacy private.pem/public.pem pair (kid kept as 'default' for token compatibility)"),
                );
                tracing::info!(
                    path = %keystore_path.display(),
                    kid = %ks.data.active_kid,
                    "imported legacy JWT key pair into keystore (kid kept as 'default' for token compatibility)"
                );
                Ok(ks)
            }
            (None, None) => {
                let ks = Self::generate_fresh(keystore_path, min_retention_sec)?;
                tracing::info!(
                    path = %keystore_path.display(),
                    kid = %ks.data.active_kid,
                    "no existing JWT keys found; generated fresh key pair (first-boot initialization)"
                );
                Ok(ks)
            }
            _ => Err(KeyStoreError::Load(format!(
                "legacy key files half-missing (private: {legacy_private_path}, public: {legacy_public_path}); \
                 refusing to start — restore the missing file from backup or remove both and the keystore to re-initialize"
            ))),
        }
    }

    /// Fail-closed loader: corrupt JSON, unparseable PEMs, private/public
    /// mismatch or permissive unix permissions all abort startup.
    pub fn load(path: &Path, min_retention_sec: i64) -> Result<Self, KeyStoreError> {
        #[cfg(unix)]
        check_private_file_perms(path)?;
        let raw = std::fs::read_to_string(path)
            .map_err(|e| KeyStoreError::Load(format!("read {}: {e}", path.display())))?;
        let data: KeyStoreData = serde_json::from_str(&raw)
            .map_err(|e| KeyStoreError::Load(format!("parse {}: {e}", path.display())))?;
        validate_data(&data)?;
        tracing::info!(
            path = %path.display(),
            keys = data.keys.len(),
            active_kid = %data.active_kid,
            "loaded JWT keystore"
        );
        Ok(Self {
            path: path.to_path_buf(),
            min_retention_sec,
            data,
            audit: AuditLedger::beside_keystore(path),
            fingerprint: Some(sha256_hex(raw.as_bytes())),
        })
    }

    /// First-boot initialization: generate a new RSA-2048 key pair with a
    /// fingerprint-derived kid and persist it atomically.
    pub fn generate_fresh(path: &Path, min_retention_sec: i64) -> Result<Self, KeyStoreError> {
        let (private_pem, public_pem) = generate_rsa_pair()?;
        let kid = derive_kid(&public_pem)?;
        let data = KeyStoreData {
            version: KEYSTORE_VERSION,
            active_kid: kid.clone(),
            keys: vec![KeyEntry {
                kid,
                status: KeyStatus::Active,
                created_at: now_sec(),
                retired_at: None,
                private_pem,
                public_pem,
            }],
        };
        let mut ks = Self {
            path: path.to_path_buf(),
            min_retention_sec,
            data,
            audit: AuditLedger::beside_keystore(path),
            fingerprint: None,
        };
        ks.save()?;
        ks.audit.record(
            &AuditContext::system(),
            AuditAction::Create,
            AuditResult::Success,
            Some(&ks.data.active_kid),
            None,
            Some("first-boot initialization"),
        );
        Ok(ks)
    }

    /// Wrap an already-parsed legacy PEM pair (no disk I/O; used by `init`
    /// and tests). kid is `LEGACY_KID` ("default") for token compatibility.
    pub fn from_legacy_pems(
        private_pem: String,
        public_pem: String,
        path: &Path,
        min_retention_sec: i64,
    ) -> Result<Self, KeyStoreError> {
        let entry = build_entry(LEGACY_KID.to_string(), private_pem, public_pem)?;
        let active_kid = entry.kid.clone();
        let data = KeyStoreData {
            version: KEYSTORE_VERSION,
            active_kid,
            keys: vec![entry],
        };
        validate_data(&data)?;
        Ok(Self {
            path: path.to_path_buf(),
            min_retention_sec,
            data,
            audit: AuditLedger::beside_keystore(path),
            fingerprint: None,
        })
    }

    // ─────────────────────────────────────────────────────────────────────
    // Reads
    // ─────────────────────────────────────────────────────────────────────

    pub fn data(&self) -> &KeyStoreData {
        &self.data
    }

    pub fn min_retention_sec(&self) -> i64 {
        self.min_retention_sec
    }

    /// Durable lifecycle audit ledger (see [`crate::audit`]).
    pub fn audit_ledger(&self) -> &AuditLedger {
        &self.audit
    }

    /// The currently signing key entry.
    pub fn active(&self) -> &KeyEntry {
        self.data
            .keys
            .iter()
            .find(|k| k.kid == self.data.active_kid)
            .expect("validated: active_kid always present")
    }

    /// Entries to publish in JWKS: `active` + `retiring` (retired keys are
    /// no longer accepted by verifiers that refresh from JWKS).
    pub fn public_entries(&self) -> Vec<&KeyEntry> {
        self.data
            .keys
            .iter()
            .filter(|k| matches!(k.status, KeyStatus::Active | KeyStatus::Retiring))
            .collect()
    }

    /// Public PEMs for verify paths (refresh-token decode tries each in
    /// turn: active first, then retiring).
    pub fn verify_public_pems(&self) -> Vec<String> {
        self.public_entries()
            .into_iter()
            .map(|k| k.public_pem.clone())
            .collect()
    }

    // ─────────────────────────────────────────────────────────────────────
    // Mutations (each one persists atomically before returning)
    // ─────────────────────────────────────────────────────────────────────

    /// Rotate: generate a new key that becomes the only signer; the previous
    /// active key downgrades to `retiring` (verify-only, still in JWKS) so
    /// outstanding tokens keep working. Returns `(new_kid, previous_kid)`.
    pub fn rotate(
        &mut self,
        now_sec: i64,
        ctx: &AuditContext,
    ) -> Result<(String, String), KeyStoreError> {
        match self.rotate_inner(now_sec) {
            Ok((new_kid, previous_kid)) => {
                self.audit.record(
                    ctx,
                    AuditAction::Rotate,
                    AuditResult::Success,
                    Some(&new_kid),
                    Some(&previous_kid),
                    None,
                );
                Ok((new_kid, previous_kid))
            }
            Err(e) => {
                self.audit.record(
                    ctx,
                    AuditAction::Rotate,
                    AuditResult::Failed,
                    Some(&self.data.active_kid),
                    None,
                    Some(&e.to_string()),
                );
                Err(e)
            }
        }
    }

    fn rotate_inner(&mut self, now_sec: i64) -> Result<(String, String), KeyStoreError> {
        let previous_kid = self.data.active_kid.clone();
        let (private_pem, public_pem) = generate_rsa_pair()?;
        let kid = derive_kid(&public_pem)?;
        if self.data.keys.iter().any(|k| k.kid == kid) {
            // Astronomically unlikely (128-bit collision) but fail closed.
            return Err(KeyStoreError::Generate(
                "generated key kid collides with an existing entry".into(),
            ));
        }
        for k in &mut self.data.keys {
            if k.status == KeyStatus::Active {
                k.status = KeyStatus::Retiring;
            }
        }
        self.data.keys.push(KeyEntry {
            kid: kid.clone(),
            status: KeyStatus::Active,
            created_at: now_sec,
            retired_at: None,
            private_pem,
            public_pem,
        });
        self.data.active_kid = kid.clone();
        self.save()?;
        tracing::info!(
            new_kid = %kid,
            previous_kid = %previous_kid,
            "rotated JWT signing key; previous key moved to 'retiring' (verify-only, still in JWKS)"
        );
        Ok((kid, previous_kid))
    }

    /// `retiring` → `retired`: stops publishing the key in JWKS. Only
    /// allowed on retiring keys — the single `active` signer can never be
    /// retired directly (that would orphan every future token).
    pub fn retire(
        &mut self,
        kid: &str,
        now_sec: i64,
        ctx: &AuditContext,
    ) -> Result<(), KeyStoreError> {
        match self.retire_inner(kid, now_sec) {
            Ok(()) => {
                self.audit.record(
                    ctx,
                    AuditAction::Retire,
                    AuditResult::Success,
                    Some(kid),
                    None,
                    None,
                );
                Ok(())
            }
            Err(e) => {
                self.audit.record(
                    ctx,
                    AuditAction::Retire,
                    AuditResult::Failed,
                    Some(kid),
                    None,
                    Some(&e.to_string()),
                );
                Err(e)
            }
        }
    }

    fn retire_inner(&mut self, kid: &str, now_sec: i64) -> Result<(), KeyStoreError> {
        let k = self
            .data
            .keys
            .iter_mut()
            .find(|k| k.kid == kid)
            .ok_or_else(|| KeyStoreError::InvalidOperation(format!("unknown kid {kid}")))?;
        if k.status != KeyStatus::Retiring {
            return Err(KeyStoreError::InvalidOperation(
                "only 'retiring' keys can be retired (rotate first to demote the active key)"
                    .to_string(),
            ));
        }
        k.status = KeyStatus::Retired;
        k.retired_at = Some(now_sec);
        self.save()?;
        tracing::info!(kid = %kid, "retired JWT key; removed from JWKS");
        Ok(())
    }

    /// Delete a key entry entirely. Only `retired` keys past
    /// `min_retention_sec` since retirement are deletable — until every
    /// token signed by the key has expired, the entry must stay (retention
    /// default = refresh TTL, so nothing verifiable is ever destroyed).
    pub fn remove(
        &mut self,
        kid: &str,
        now_sec: i64,
        ctx: &AuditContext,
    ) -> Result<(), KeyStoreError> {
        match self.remove_inner(kid, now_sec) {
            Ok(()) => {
                self.audit.record(
                    ctx,
                    AuditAction::Delete,
                    AuditResult::Success,
                    Some(kid),
                    None,
                    None,
                );
                Ok(())
            }
            Err(e) => {
                self.audit.record(
                    ctx,
                    AuditAction::Delete,
                    AuditResult::Failed,
                    Some(kid),
                    None,
                    Some(&e.to_string()),
                );
                Err(e)
            }
        }
    }

    fn remove_inner(&mut self, kid: &str, now_sec: i64) -> Result<(), KeyStoreError> {
        let idx = self
            .data
            .keys
            .iter()
            .position(|k| k.kid == kid)
            .ok_or_else(|| KeyStoreError::InvalidOperation(format!("unknown kid {kid}")))?;
        let k = &self.data.keys[idx];
        if kid == self.data.active_kid {
            return Err(KeyStoreError::InvalidOperation(
                "cannot delete the active signing key".to_string(),
            ));
        }
        if k.status != KeyStatus::Retired {
            return Err(KeyStoreError::InvalidOperation(
                "only 'retired' keys can be deleted (retire it first)".to_string(),
            ));
        }
        let retired_at = k.retired_at.ok_or_else(|| {
            KeyStoreError::InvalidOperation("retired key missing retired_at".into())
        })?;
        let elapsed = now_sec - retired_at;
        if elapsed < self.min_retention_sec {
            return Err(KeyStoreError::InvalidOperation(format!(
                "retention not satisfied: {}s elapsed < {}s required; wait before deleting (tokens signed by this kid may not all be expired yet)",
                elapsed.max(0), self.min_retention_sec
            )));
        }
        self.data.keys.remove(idx);
        self.save()?;
        tracing::info!(kid = %kid, "deleted JWT key entry past retention");
        Ok(())
    }

    /// Persist to disk atomically: write sibling `.tmp`, fsync, rename.
    /// A crash mid-write can never corrupt the live keystore.
    ///
    /// Serialised through the cross-process lock file and guarded by the
    /// disk fingerprint: if another process rewrote the keystore since this
    /// snapshot was loaded, the write fails with [`KeyStoreError::Stale`]
    /// instead of silently deleting the other process' key changes.
    pub fn save(&mut self) -> Result<(), KeyStoreError> {
        let _lock = KeystoreLock::acquire(&self.path)?;
        self.persist_locked()
    }

    /// Caller already holds the cross-process lock.
    fn persist_locked(&mut self) -> Result<(), KeyStoreError> {
        let json = serde_json::to_string_pretty(&self.data)
            .map_err(|e| KeyStoreError::Save(format!("serialize keystore: {e}")))?;
        let current = file_fingerprint(&self.path)?;
        if current != self.fingerprint {
            return Err(KeyStoreError::Stale(format!(
                "{} changed on disk since it was loaded; reload before retrying \
                 (a concurrent keytool run or another instance wrote it — refusing to overwrite)",
                self.path.display()
            )));
        }
        atomic_write(&self.path, &json)?;
        self.fingerprint = Some(sha256_hex(json.as_bytes()));
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────
    // Backup / restore
    // ─────────────────────────────────────────────────────────────────────

    /// Write a backup file (0600 on unix) wrapped in a versioned,
    /// checksummed envelope. The backup contains all private key material —
    /// operators must store it in protected storage and keep it out of git
    /// (`*.backup.json` is gitignored for this reason).
    pub fn write_backup(
        &self,
        backup_path: &Path,
        ctx: &AuditContext,
    ) -> Result<(), KeyStoreError> {
        match self.write_backup_inner(backup_path) {
            Ok(()) => {
                self.audit.record(
                    ctx,
                    AuditAction::Backup,
                    AuditResult::Success,
                    Some(&self.data.active_kid),
                    None,
                    Some(&format!(
                        "wrote backup ({} keys) to {}",
                        self.data.keys.len(),
                        backup_path.display()
                    )),
                );
                Ok(())
            }
            Err(e) => {
                self.audit.record(
                    ctx,
                    AuditAction::Backup,
                    AuditResult::Failed,
                    Some(&self.data.active_kid),
                    None,
                    Some(&e.to_string()),
                );
                Err(e)
            }
        }
    }

    fn write_backup_inner(&self, backup_path: &Path) -> Result<(), KeyStoreError> {
        let envelope = BackupEnvelope {
            backup_format_version: BACKUP_FORMAT_VERSION,
            created_at: now_sec(),
            producer: "token-service".to_string(),
            keystore_sha256: keystore_checksum(&self.data)?,
            keystore: self.data.clone(),
        };
        let json = serde_json::to_string_pretty(&envelope)
            .map_err(|e| KeyStoreError::Save(format!("serialize backup envelope: {e}")))?;
        atomic_write(backup_path, &json)?;
        tracing::info!(
            path = %backup_path.display(),
            keys = self.data.keys.len(),
            "wrote keystore backup (versioned envelope with SHA-256 integrity checksum)"
        );
        Ok(())
    }

    /// Restore a backup over the live keystore path.
    ///
    /// Safety sequence: envelope version → SHA-256 integrity → full key
    /// validation → cross-process lock → pre-restore snapshot of the live
    /// keystore → atomic overwrite (stale-write guarded) → read-back
    /// verification. Any failure after the overwrite rolls the previous
    /// keystore back, so a failed restore never destroys the live state.
    /// After a successful restore, tokens signed by the restored keys verify
    /// again.
    pub fn restore_from_backup(
        backup_path: &Path,
        keystore_path: &Path,
        min_retention_sec: i64,
        ctx: &AuditContext,
    ) -> Result<Self, KeyStoreError> {
        Self::restore_impl(backup_path, keystore_path, min_retention_sec, ctx, false)
    }

    /// Test-only entry point: same as [`Self::restore_from_backup`] but the
    /// post-write verification is forced to fail so the rollback path can be
    /// exercised deterministically.
    #[cfg(test)]
    pub(crate) fn restore_from_backup_with_verification_failure(
        backup_path: &Path,
        keystore_path: &Path,
        min_retention_sec: i64,
        ctx: &AuditContext,
    ) -> Result<Self, KeyStoreError> {
        Self::restore_impl(backup_path, keystore_path, min_retention_sec, ctx, true)
    }

    fn restore_impl(
        backup_path: &Path,
        keystore_path: &Path,
        min_retention_sec: i64,
        ctx: &AuditContext,
        fail_verification: bool,
    ) -> Result<Self, KeyStoreError> {
        let ledger = AuditLedger::beside_keystore(keystore_path);
        match Self::restore_inner(
            backup_path,
            keystore_path,
            min_retention_sec,
            fail_verification,
        ) {
            Ok(ks) => {
                ks.audit.record(
                    ctx,
                    AuditAction::Restore,
                    AuditResult::Success,
                    Some(&ks.data.active_kid),
                    None,
                    Some(&format!(
                        "restored {} keys from {}",
                        ks.data.keys.len(),
                        backup_path.display()
                    )),
                );
                Ok(ks)
            }
            Err(e) => {
                ledger.record(
                    ctx,
                    AuditAction::Restore,
                    AuditResult::Failed,
                    None,
                    None,
                    Some(&e.to_string()),
                );
                Err(e)
            }
        }
    }

    fn restore_inner(
        backup_path: &Path,
        keystore_path: &Path,
        min_retention_sec: i64,
        fail_verification: bool,
    ) -> Result<Self, KeyStoreError> {
        let raw = std::fs::read_to_string(backup_path).map_err(|e| {
            KeyStoreError::BackupIntegrity(format!("read backup {}: {e}", backup_path.display()))
        })?;
        let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
            KeyStoreError::BackupIntegrity(format!("parse backup {}: {e}", backup_path.display()))
        })?;
        if value.get("keystore").is_none() {
            return Err(KeyStoreError::BackupVersion(format!(
                "{}: legacy bare-keystore backup (no envelope, no checksum) is not supported; \
                 re-create it with `keytool backup`",
                backup_path.display()
            )));
        }
        let envelope: BackupEnvelope = serde_json::from_value(value).map_err(|e| {
            KeyStoreError::BackupIntegrity(format!(
                "parse backup envelope {}: {e}",
                backup_path.display()
            ))
        })?;
        if envelope.backup_format_version != BACKUP_FORMAT_VERSION {
            return Err(KeyStoreError::BackupVersion(format!(
                "{}: backup_format_version {} (this build supports {BACKUP_FORMAT_VERSION})",
                backup_path.display(),
                envelope.backup_format_version
            )));
        }
        let computed = keystore_checksum(&envelope.keystore)?;
        if computed != envelope.keystore_sha256 {
            return Err(KeyStoreError::BackupIntegrity(format!(
                "{}: keystore_sha256 mismatch (recorded {}, computed {}) — backup is corrupted or was modified",
                backup_path.display(),
                envelope.keystore_sha256,
                computed
            )));
        }
        validate_data(&envelope.keystore)?;

        // Everything verified — take the write lock before touching the disk.
        let _lock = KeystoreLock::acquire(keystore_path)?;
        let current_fingerprint = file_fingerprint(keystore_path)?;
        // Safety net: keep the pre-restore keystore so a failed restore can
        // be rolled back (and an operator can undo a semantically wrong one).
        let snapshot = if current_fingerprint.is_some() {
            let bytes = std::fs::read(keystore_path).map_err(|e| {
                KeyStoreError::RestoreFailed(format!(
                    "read current keystore for pre-restore snapshot: {e}"
                ))
            })?;
            let snap_path = pre_restore_snapshot_path(keystore_path);
            atomic_write_bytes(&snap_path, &bytes)?;
            tracing::warn!(
                path = %snap_path.display(),
                "wrote pre-restore keystore snapshot (0600) before overwriting"
            );
            Some(snap_path)
        } else {
            None
        };

        let mut ks = Self {
            path: keystore_path.to_path_buf(),
            min_retention_sec,
            data: envelope.keystore,
            audit: AuditLedger::beside_keystore(keystore_path),
            fingerprint: current_fingerprint,
        };
        ks.persist_locked()?;

        // Read back what is actually on disk; a restore that cannot be read
        // back is not a restore.
        let verification = match Self::load(keystore_path, min_retention_sec) {
            Ok(loaded)
                if loaded.data.active_kid == ks.data.active_kid
                    && loaded.data.keys.len() == ks.data.keys.len() =>
            {
                Ok(())
            }
            Ok(_) => Err(KeyStoreError::RestoreFailed(
                "post-restore read-back does not match the backup (active kid / key count)".into(),
            )),
            Err(e) => Err(KeyStoreError::RestoreFailed(format!(
                "post-restore read-back failed: {e}"
            ))),
        };
        let verification = if fail_verification {
            Err(KeyStoreError::RestoreFailed(
                "injected post-restore verification failure (test)".into(),
            ))
        } else {
            verification
        };
        if let Err(e) = verification {
            if let Some(snap) = &snapshot {
                let bytes = std::fs::read(snap).map_err(|re| {
                    KeyStoreError::RestoreFailed(format!(
                        "{e}; additionally failed to read the pre-restore snapshot: {re}"
                    ))
                })?;
                atomic_write_bytes(keystore_path, &bytes)?;
                ks.fingerprint = file_fingerprint(keystore_path)?;
                tracing::error!(
                    error = %e,
                    snapshot = %snap.display(),
                    "restore verification failed; rolled back to the pre-restore snapshot"
                );
            }
            return Err(e);
        }
        tracing::info!(
            backup = %backup_path.display(),
            path = %keystore_path.display(),
            active_kid = %ks.data.active_kid,
            "restored keystore from backup"
        );
        Ok(ks)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn now_sec() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Read a file that may legitimately be absent (`None` = NotFound). Any
/// other IO error is a hard failure — a permission error on the legacy key
/// must not look like "no key here" and trigger regeneration.
fn read_opt(path: &str) -> Result<Option<String>, KeyStoreError> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(KeyStoreError::Load(format!("read {path}: {e}"))),
    }
}

/// Structural + cryptographic validation shared by `load` and `restore`.
/// Every private PEM must parse, every public PEM must parse, and each
/// key's public half must actually be the public half of its private half —
/// a half-edited keystore must fail closed instead of signing with a key
/// whose published JWKS half belongs to something else.
fn validate_data(data: &KeyStoreData) -> Result<(), KeyStoreError> {
    if data.version != KEYSTORE_VERSION {
        return Err(KeyStoreError::Load(format!(
            "unsupported keystore version {} (expected {KEYSTORE_VERSION})",
            data.version
        )));
    }
    if data.keys.is_empty() {
        return Err(KeyStoreError::Load("keystore has no keys".into()));
    }
    let active = data
        .keys
        .iter()
        .find(|k| k.kid == data.active_kid)
        .ok_or_else(|| KeyStoreError::Load("active_kid not found among keys".into()))?;
    if active.status != KeyStatus::Active {
        return Err(KeyStoreError::Load(
            "active_kid entry is not in 'active' status".into(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for k in &data.keys {
        if !seen.insert(&k.kid) {
            return Err(KeyStoreError::Load(format!("duplicate kid {}", k.kid)));
        }
        if k.kid.is_empty() {
            return Err(KeyStoreError::Load("empty kid entry".into()));
        }
        let pair = RS256KeyPair::from_pem(&k.private_pem)
            .map_err(|e| KeyStoreError::Load(format!("kid {}: private PEM: {e}", k.kid)))?;
        let pub_key = RS256PublicKey::from_pem(&k.public_pem)
            .map_err(|e| KeyStoreError::Load(format!("kid {}: public PEM: {e}", k.kid)))?;
        let priv_pub_der = pair
            .public_key()
            .to_der()
            .map_err(|e| KeyStoreError::Load(format!("kid {}: derive public: {e}", k.kid)))?;
        let stored_der = pub_key
            .to_der()
            .map_err(|e| KeyStoreError::Load(format!("kid {}: encode public: {e}", k.kid)))?;
        if priv_pub_der != stored_der {
            return Err(KeyStoreError::Load(format!(
                "kid {}: stored public key does not match the private key",
                k.kid
            )));
        }
        if k.status == KeyStatus::Retired && k.retired_at.is_none() {
            return Err(KeyStoreError::Load(format!(
                "kid {}: retired key missing retired_at",
                k.kid
            )));
        }
    }
    Ok(())
}

fn build_entry(
    kid: String,
    private_pem: String,
    public_pem: String,
) -> Result<KeyEntry, KeyStoreError> {
    // Parse here so imports of corrupt PEMs fail before any disk write,
    // and cross-check that the two halves belong to the same key.
    let pair = RS256KeyPair::from_pem(&private_pem)
        .map_err(|e| KeyStoreError::Load(format!("legacy private PEM: {e}")))?;
    let stored_pub = RS256PublicKey::from_pem(&public_pem)
        .map_err(|e| KeyStoreError::Load(format!("legacy public PEM: {e}")))?;
    let derived = pair
        .public_key()
        .to_der()
        .map_err(|e| KeyStoreError::Load(format!("derive public: {e}")))?;
    let stored_der = stored_pub
        .to_der()
        .map_err(|e| KeyStoreError::Load(format!("encode public: {e}")))?;
    if derived != stored_der {
        return Err(KeyStoreError::Load(
            "legacy public.pem does not match legacy private.pem".into(),
        ));
    }
    Ok(KeyEntry {
        kid,
        status: KeyStatus::Active,
        created_at: now_sec(),
        retired_at: None,
        private_pem,
        public_pem,
    })
}

fn generate_rsa_pair() -> Result<(String, String), KeyStoreError> {
    let key_pair = RS256KeyPair::generate(2048)
        .map_err(|e| KeyStoreError::Generate(format!("RSA generate: {e}")))?;
    let private_pem = key_pair
        .to_pem()
        .map_err(|e| KeyStoreError::Generate(format!("encode private PEM: {e}")))?;
    let public_pem = key_pair
        .public_key()
        .to_pem()
        .map_err(|e| KeyStoreError::Generate(format!("encode public PEM: {e}")))?;
    Ok((private_pem, public_pem))
}

/// kid = first 16 bytes of SHA-256 over the public key DER, hex-encoded
/// (32 chars). Content-addressed: the same key always yields the same kid
/// across restarts, containers and backups.
pub fn derive_kid(public_pem: &str) -> Result<String, KeyStoreError> {
    let pub_key = RS256PublicKey::from_pem(public_pem)
        .map_err(|e| KeyStoreError::Load(format!("derive kid: {e}")))?;
    let der = pub_key
        .to_der()
        .map_err(|e| KeyStoreError::Load(format!("derive kid: {e}")))?;
    let hash = Sha256::digest(&der);
    Ok(hex::encode(&hash[..16]))
}

/// Lowercase hex SHA-256 of `bytes`.
fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Integrity checksum of a keystore payload (compact JSON, deterministic
/// field order because it is a struct).
fn keystore_checksum(data: &KeyStoreData) -> Result<String, KeyStoreError> {
    let canonical = serde_json::to_string(data)
        .map_err(|e| KeyStoreError::Save(format!("serialize keystore for checksum: {e}")))?;
    Ok(sha256_hex(canonical.as_bytes()))
}

/// SHA-256 of the file's current bytes; `None` when the file does not exist.
fn file_fingerprint(path: &Path) -> Result<Option<String>, KeyStoreError> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(sha256_hex(&bytes))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(KeyStoreError::Save(format!(
            "read {} for fingerprint: {e}",
            path.display()
        ))),
    }
}

/// `keystore.pre-restore.<epoch>.json` next to the keystore.
fn pre_restore_snapshot_path(keystore_path: &Path) -> PathBuf {
    let stamp = chrono::Utc::now().timestamp();
    let stem = keystore_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("keystore");
    keystore_path.with_file_name(format!("{stem}.pre-restore.{stamp}.json"))
}

/// Cross-process write lock (`keystore.json.lock`).
///
/// Created with `create_new` (atomic on every supported filesystem), removed
/// on drop. A lock file older than [`LOCK_STALE_AFTER`] belongs to a crashed
/// process and is reclaimed. This is the *only* concurrency mechanism: the
/// deployment is a single instance by design (see the runbook); the lock plus
/// the write fingerprint turn unsupported concurrent writers into explicit
/// errors instead of silent key loss.
#[derive(Debug)]
struct KeystoreLock {
    path: PathBuf,
}

impl KeystoreLock {
    fn acquire(keystore_path: &Path) -> Result<Self, KeyStoreError> {
        Self::acquire_within(keystore_path, LOCK_WAIT)
    }

    fn acquire_within(keystore_path: &Path, wait: Duration) -> Result<Self, KeyStoreError> {
        let mut lock_os = keystore_path.as_os_str().to_owned();
        lock_os.push(".lock");
        let path = PathBuf::from(lock_os);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    KeyStoreError::Save(format!("create dir {}: {e}", parent.display()))
                })?;
            }
        }
        let started = std::time::Instant::now();
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut f) => {
                    use std::io::Write;
                    let _ = write!(f, "pid={} at={}", std::process::id(), now_sec());
                    let _ = f.sync_all();
                    return Ok(Self { path });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if lock_is_stale(&path) {
                        tracing::warn!(
                            path = %path.display(),
                            "reclaiming stale keystore lock file (older than {}s)",
                            LOCK_STALE_AFTER.as_secs()
                        );
                        let _ = std::fs::remove_file(&path);
                        continue;
                    }
                    if started.elapsed() >= wait {
                        return Err(KeyStoreError::LockBusy(format!(
                            "{} held by another key operation (waited {}ms); retry after it finishes",
                            path.display(),
                            wait.as_millis()
                        )));
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(e) => {
                    return Err(KeyStoreError::Save(format!(
                        "create lock {}: {e}",
                        path.display()
                    )))
                }
            }
        }
    }
}

impl Drop for KeystoreLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn lock_is_stale(path: &Path) -> bool {
    match std::fs::metadata(path).and_then(|m| m.modified()) {
        Ok(modified) => SystemTime::now()
            .duration_since(modified)
            .map(|age| age > LOCK_STALE_AFTER)
            .unwrap_or(false),
        Err(_) => false,
    }
}

/// Write `contents` to `path` atomically: tmp file in the same directory,
/// fsync, rename over the target. Unix: file 0600, directory 0700.
fn atomic_write(path: &Path, contents: &str) -> Result<(), KeyStoreError> {
    atomic_write_bytes(path, contents.as_bytes())
}

fn atomic_write_bytes(path: &Path, contents: &[u8]) -> Result<(), KeyStoreError> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|e| KeyStoreError::Save(format!("create dir {}: {e}", parent.display())))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| KeyStoreError::Save(format!("chmod dir: {e}")))?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp)
            .map_err(|e| KeyStoreError::Save(format!("create {}: {e}", tmp.display())))?;
        f.write_all(contents)
            .map_err(|e| KeyStoreError::Save(format!("write {}: {e}", tmp.display())))?;
        f.sync_all()
            .map_err(|e| KeyStoreError::Save(format!("fsync {}: {e}", tmp.display())))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| KeyStoreError::Save(format!("chmod file: {e}")))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        KeyStoreError::Save(format!(
            "rename {} -> {}: {e}",
            tmp.display(),
            path.display()
        ))
    })?;
    Ok(())
}

/// Unix-only: the keystore holds every private key, so group/other access
/// bits are a fail-closed startup error (defense against a volume mounted
/// with the wrong umask/owner). Windows dev hosts skip this (NTFS ACLs are
/// not POSIX modes); production containers are linux/musl and always run it.
#[cfg(unix)]
fn check_private_file_perms(path: &Path) -> Result<(), KeyStoreError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path)
        .map_err(|e| KeyStoreError::Load(format!("stat {}: {e}", path.display())))?
        .permissions()
        .mode();
    if mode & 0o077 != 0 {
        return Err(KeyStoreError::UnsafePermissions(format!(
            "{}: mode {:o} — group/other must not have access (chmod 600)",
            path.display(),
            mode & 0o777
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use rsa::pkcs8::DecodePublicKey;
    use std::collections::HashMap;

    fn tmp_ks() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys").join("keystore.json");
        (dir, path)
    }

    fn fresh_pair() -> (String, String) {
        let kp = RS256KeyPair::generate(2048).unwrap();
        (kp.to_pem().unwrap(), kp.public_key().to_pem().unwrap())
    }

    fn ctx() -> AuditContext {
        AuditContext::new("test", None)
    }

    fn verify_with(pub_pem: &str, token: &str) -> bool {
        RS256PublicKey::from_pem(pub_pem)
            .unwrap()
            .verify_token::<HashMap<String, serde_json::Value>>(token, None)
            .is_ok()
    }

    fn sign_with(priv_pem: &str, kid: &str) -> String {
        RS256KeyPair::from_pem(priv_pem)
            .unwrap()
            .with_key_id(kid)
            .sign(JWTClaims::<HashMap<String, serde_json::Value>>::new())
            .unwrap()
    }

    // 1. 首次启动生成密钥
    #[test]
    fn first_boot_generates_keystore() {
        let (_d, path) = tmp_ks();
        let ks = KeyStore::init(
            &path,
            "/nonexistent/private.pem",
            "/nonexistent/public.pem",
            100,
        )
        .expect("first boot generates");
        assert_eq!(ks.data().keys.len(), 1);
        assert_eq!(ks.active().status, KeyStatus::Active);
        assert!(path.exists());
        // fresh kid is fingerprint-derived, never the legacy value
        assert_ne!(ks.active().kid, LEGACY_KID);
    }

    // 2. 再次启动加载原密钥
    #[test]
    fn second_boot_loads_same_key() {
        let (_d, path) = tmp_ks();
        let ks1 = KeyStore::init(&path, "/nonexistent/p", "/nonexistent/q", 100).unwrap();
        let ks2 = KeyStore::init(&path, "/nonexistent/p", "/nonexistent/q", 100).unwrap();
        assert_eq!(ks1.active().kid, ks2.active().kid);
        assert_eq!(ks1.active().private_pem, ks2.active().private_pem);
    }

    // 3. 容器重建后密钥不变（与 2 同语义，显式覆盖"重建 = 同目录重新 init"）
    #[test]
    fn container_recreation_keeps_key_unchanged() {
        let (_d, path) = tmp_ks();
        let before = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let after = KeyStore::init(&path, "np", "nq", 100).unwrap();
        assert_eq!(before.active().public_pem, after.active().public_pem);
    }

    // 4. 原 JWT 在重启后仍然有效
    #[test]
    fn jwt_survives_restart() {
        let (_d, path) = tmp_ks();
        let kid = {
            let ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
            let kid = ks.active().kid.clone();
            let pem = ks.active().private_pem.clone();
            let _token = sign_with(&pem, &kid);
            kid
        }; // "process restart"
        let ks2 = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let token = sign_with(&ks2.active().private_pem, &kid);
        assert!(verify_with(&ks2.active().public_pem, &token));
    }

    // 5. JWKS 返回正确的公钥（active + retiring，kid 一致，n/e 可构造）
    #[test]
    fn jwks_entries_cover_active_and_retiring_with_correct_components() {
        let (_d, path) = tmp_ks();
        let mut ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let active = ks.active();
        let key = rsa::RsaPublicKey::from_public_key_pem(&active.public_pem).unwrap();
        use rsa::traits::PublicKeyParts;
        let n = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key.n().to_bytes_be());
        let e = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key.e().to_bytes_be());
        assert_eq!(derive_kid(&active.public_pem).unwrap(), active.kid);
        assert!(!n.is_empty() && e == "AQAB");

        let (new_kid, prev_kid) = ks.rotate(now_sec(), &ctx()).unwrap();
        let entries = ks.public_entries();
        assert_eq!(entries.len(), 2);
        let kids: Vec<&str> = entries.iter().map(|k| k.kid.as_str()).collect();
        assert!(kids.contains(&new_kid.as_str()) && kids.contains(&prev_kid.as_str()));
    }

    // 6. 密钥轮换后新旧 JWT 均可验证
    #[test]
    fn rotation_keeps_old_tokens_verifiable() {
        let (_d, path) = tmp_ks();
        let mut ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let old = ks.active().clone();
        let old_token = sign_with(&old.private_pem, &old.kid);

        let (new_kid, _prev) = ks.rotate(now_sec(), &ctx()).unwrap();
        let new_token = sign_with(&ks.active().private_pem, &new_kid);

        // every JWKS-published public key can verify the OLD token
        assert!(ks
            .verify_public_pems()
            .iter()
            .any(|p| verify_with(p, &old_token)));
        // and the NEW token verifies against the active public key
        assert!(verify_with(&ks.active().public_pem, &new_token));
        // signing moved: new kid differs from old
        assert_ne!(new_kid, old.kid);
        assert_eq!(ks.active().kid, new_kid);
    }

    // 7. 旧密钥退役后行为符合预期（retire 后出 JWKS；remove 需满足保留期）
    #[test]
    fn retire_then_remove_lifecycle() {
        let (_d, path) = tmp_ks();
        let mut ks = KeyStore::init(&path, "np", "nq", 1000).unwrap();
        let old_kid = ks.active().kid.clone();
        let (_, prev) = ks.rotate(now_sec(), &ctx()).unwrap();
        assert_eq!(prev, old_kid);

        // active signer cannot be retired or deleted directly
        assert!(ks
            .retire(&ks.active().kid.clone(), now_sec(), &ctx())
            .is_err());
        assert!(ks
            .remove(&ks.active().kid.clone(), now_sec(), &ctx())
            .is_err());

        // retiring → retired leaves JWKS
        ks.retire(&prev, now_sec(), &ctx()).unwrap();
        assert_eq!(ks.public_entries().len(), 1);
        assert!(ks.public_entries().iter().all(|k| k.kid == ks.active().kid));

        // retention gate blocks deletion, then allows it after retention
        let err = ks.remove(&prev, now_sec(), &ctx()).unwrap_err();
        assert!(err.to_string().contains("retention not satisfied"));
        ks.remove(&prev, now_sec() + 1000, &ctx()).unwrap();
        assert!(ks.data().keys.iter().all(|k| k.kid != prev));
    }

    // 8. 密钥文件损坏时安全失败，不得静默生成新密钥
    #[test]
    fn corrupt_keystore_fails_closed_without_regenerating() {
        let (_d, path) = tmp_ks();
        let original = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let original_pem = original.active().private_pem.clone();

        std::fs::write(&path, "{ this is not json").unwrap();
        let err = KeyStore::init(&path, "np", "nq", 100).unwrap_err();
        assert!(matches!(err, KeyStoreError::Load(_)));
        // file untouched (no silent rewrite)
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{ this is not json"
        );
        // and the in-memory key from before the corruption still works —
        // the operator can restore, not re-mint everything
        assert!(verify_with(
            &original.active().public_pem,
            &sign_with(&original_pem, &original.active().kid)
        ));
    }

    #[test]
    fn corrupt_pem_inside_keystore_fails_closed() {
        let (_d, path) = tmp_ks();
        KeyStore::init(&path, "np", "nq", 100).unwrap();
        let mut data: KeyStoreData =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        data.keys[0].private_pem =
            "-----BEGIN PRIVATE KEY-----\nbroken\n-----END PRIVATE KEY-----\n".into();
        std::fs::write(&path, serde_json::to_string(&data).unwrap()).unwrap();
        assert!(matches!(
            KeyStore::load(&path, 100),
            Err(KeyStoreError::Load(_))
        ));
    }

    #[test]
    fn mismatched_private_public_pair_fails_closed() {
        let (_d, path) = tmp_ks();
        let (priv_a, _) = fresh_pair();
        let (_, pub_b) = fresh_pair();
        let ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let mut data = ks.data().clone();
        data.keys[0].private_pem = priv_a;
        data.keys[0].public_pem = pub_b;
        std::fs::write(&path, serde_json::to_string(&data).unwrap()).unwrap();
        assert!(KeyStore::load(&path, 100).is_err());
    }

    // 9. 密钥文件权限不符合要求时安全失败（unix；CI linux 执行）
    #[cfg(unix)]
    #[test]
    fn permissive_key_file_fails_closed() {
        use std::os::unix::fs::PermissionsExt;
        let (_d, path) = tmp_ks();
        KeyStore::init(&path, "np", "nq", 100).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let err = KeyStore::load(&path, 100).unwrap_err();
        assert!(matches!(err, KeyStoreError::UnsafePermissions(_)));
        // generated files carry tight perms in the first place
        let (_d2, path2) = tmp_ks();
        KeyStore::generate_fresh(&path2, 100).unwrap();
        let mode = std::fs::metadata(&path2).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    // 10. 备份恢复后原 JWT 仍然有效
    #[test]
    fn backup_restore_preserves_old_tokens() {
        let (_d, path) = tmp_ks();
        let ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let token = sign_with(&ks.active().private_pem, &ks.active().kid);
        let backup_path = _d.path().join("keys.backup.json");
        ks.write_backup(&backup_path, &ctx()).unwrap();

        // simulate total loss: keystore gone, fresh empty dir
        std::fs::remove_file(&path).unwrap();
        let restored = KeyStore::restore_from_backup(&backup_path, &path, 100, &ctx()).unwrap();
        assert_eq!(restored.active().kid, ks.active().kid);
        assert!(verify_with(&restored.active().public_pem, &token));
        // and the restored keystore boots cleanly afterwards
        let rebooted = KeyStore::init(&path, "np", "nq", 100).unwrap();
        assert_eq!(rebooted.active().kid, ks.active().kid);
    }

    #[test]
    fn restore_rejects_corrupt_backup_without_touching_live_keystore() {
        let (_d, path) = tmp_ks();
        let ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let before = std::fs::read_to_string(&path).unwrap();
        let bad = _d.path().join("bad.backup.json");
        std::fs::write(&bad, "not-json").unwrap();
        assert!(KeyStore::restore_from_backup(&bad, &path, 100, &ctx()).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
        let _ = ks;
    }

    // 迁移：旧 PEM 对导入后 kid 保持 "default"，旧 token 继续可验证
    #[test]
    fn legacy_pair_import_keeps_default_kid_and_tokens_work() {
        let dir = tempfile::tempdir().unwrap();
        let (priv_pem, pub_pem) = fresh_pair();
        let priv_path = dir.path().join("private.pem");
        let pub_path = dir.path().join("public.pem");
        std::fs::write(&priv_path, &priv_pem).unwrap();
        std::fs::write(&pub_path, &pub_pem).unwrap();

        let token = sign_with(&priv_pem, LEGACY_KID);
        let ks_path = dir.path().join("keystore.json");
        let ks = KeyStore::init(
            &ks_path,
            priv_path.to_str().unwrap(),
            pub_path.to_str().unwrap(),
            100,
        )
        .expect("import legacy pair");
        assert_eq!(ks.active().kid, LEGACY_KID);
        assert!(ks_path.exists(), "keystore persisted after import");
        // old token (kid="default") verifies against the imported key
        assert!(verify_with(&ks.active().public_pem, &token));
        // second boot loads the keystore, not a regeneration
        let ks2 = KeyStore::init(
            &ks_path,
            priv_path.to_str().unwrap(),
            pub_path.to_str().unwrap(),
            100,
        )
        .unwrap();
        assert_eq!(ks2.active().kid, LEGACY_KID);
        assert_eq!(ks2.active().private_pem, priv_pem);
    }

    // 半缺的旧 PEM 对 → fail-closed，绝不生成新钥覆盖
    #[test]
    fn half_missing_legacy_pair_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let (priv_pem, _pub) = fresh_pair();
        let priv_path = dir.path().join("private.pem");
        std::fs::write(&priv_path, &priv_pem).unwrap();
        let ks_path = dir.path().join("keystore.json");

        let err = KeyStore::init(
            &ks_path,
            priv_path.to_str().unwrap(),
            "missing-public.pem",
            100,
        )
        .unwrap_err();
        assert!(err.to_string().contains("half-missing"));
        assert!(!ks_path.exists(), "no keystore was silently created");
        assert_eq!(
            std::fs::read_to_string(&priv_path).unwrap(),
            priv_pem,
            "existing private key untouched"
        );
    }

    // 损坏的旧 PEM（读得出但解析失败）→ fail-closed（旧版会静默覆盖）
    #[test]
    fn corrupt_legacy_pem_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let priv_path = dir.path().join("private.pem");
        let pub_path = dir.path().join("public.pem");
        std::fs::write(&priv_path, "not a pem at all").unwrap();
        std::fs::write(&pub_path, "also not a pem").unwrap();
        let ks_path = dir.path().join("keystore.json");
        let err = KeyStore::init(
            &ks_path,
            priv_path.to_str().unwrap(),
            pub_path.to_str().unwrap(),
            100,
        )
        .unwrap_err();
        assert!(matches!(err, KeyStoreError::Load(_)));
        assert!(!ks_path.exists());
        // original files were not overwritten
        assert_eq!(
            std::fs::read_to_string(&priv_path).unwrap(),
            "not a pem at all"
        );
    }

    #[test]
    fn kid_is_stable_across_pem_reencoding() {
        let (priv_pem, pub_pem) = fresh_pair();
        let kid1 = derive_kid(&pub_pem).unwrap();
        // re-derive from the private half's public component
        let kp = RS256KeyPair::from_pem(&priv_pem).unwrap();
        let kid2 = derive_kid(&kp.public_key().to_pem().unwrap()).unwrap();
        assert_eq!(kid1, kid2);
        assert_eq!(kid1.len(), 32);
    }

    #[test]
    fn rotate_persists_and_reload_is_consistent() {
        let (_d, path) = tmp_ks();
        let mut ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let (new_kid, _prev) = ks.rotate(now_sec(), &ctx()).unwrap();
        let reloaded = KeyStore::load(&path, 100).unwrap();
        assert_eq!(reloaded.active().kid, new_kid);
        assert_eq!(reloaded.data().keys.len(), 2);
    }

    // ── PR #9: audit ledger ──────────────────────────────────────────────

    #[test]
    fn lifecycle_audit_is_persisted_and_never_contains_key_material() {
        let (_d, path) = tmp_ks();
        let mut ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let (new_kid, prev_kid) = ks.rotate(now_sec(), &ctx()).unwrap();
        ks.retire(&prev_kid, now_sec(), &ctx()).unwrap();
        // retention gate refusal must still be audited
        assert!(ks.remove(&prev_kid, now_sec(), &ctx()).is_err());

        let ledger = AuditLedger::beside_keystore(&path);
        let records = ledger.read_all().unwrap();
        let has = |action: &str, result: &str, actor: &str| {
            records.iter().any(|r| {
                r.payload.action == action && r.payload.result == result && r.payload.actor == actor
            })
        };
        assert!(has("create", "success", "system"));
        assert!(has("rotate", "success", "test"));
        assert!(has("retire", "success", "test"));
        assert!(has("delete", "failed", "test"));

        let rotate = records
            .iter()
            .find(|r| r.payload.action == "rotate")
            .expect("rotate audited");
        assert_eq!(rotate.payload.kid.as_deref(), Some(new_kid.as_str()));
        assert_eq!(
            rotate.payload.related_kid.as_deref(),
            Some(prev_kid.as_str())
        );
        assert_eq!(rotate.event_type, "token.jwt_key.rotate");

        // Persisted durably and readable by a fresh view (process restart).
        let reread = AuditLedger::beside_keystore(&path).read_all().unwrap();
        assert_eq!(reread.len(), records.len());

        // Never any key material, in any form.
        let raw = std::fs::read_to_string(ledger.path()).unwrap();
        assert!(!raw.contains("BEGIN"));
        assert!(!raw.contains("PRIVATE KEY"));
        assert!(!raw.contains(&ks.active().private_pem));
    }

    // ── PR #9: concurrency / multi-instance safety ───────────────────────

    #[test]
    fn stale_snapshot_cannot_overwrite_a_newer_rotation() {
        let (_d, path) = tmp_ks();
        let mut instance_a = KeyStore::init(&path, "np", "nq", 100).unwrap();
        // second "instance" loads the same file into memory
        let mut instance_b = KeyStore::load(&path, 100).unwrap();

        let (kid_a, _) = instance_a.rotate(now_sec(), &ctx()).unwrap();
        // B's snapshot is stale → must fail instead of clobbering A's key
        let err = instance_b.rotate(now_sec(), &ctx()).unwrap_err();
        assert!(matches!(err, KeyStoreError::Stale(_)));

        // Disk still holds A's rotation; nothing was lost.
        let on_disk = KeyStore::load(&path, 100).unwrap();
        assert_eq!(on_disk.active().kid, kid_a);
        assert!(on_disk.data().keys.iter().any(|k| k.kid == kid_a));
        // The failed rotation is audited too.
        let records = AuditLedger::beside_keystore(&path).read_all().unwrap();
        assert!(records
            .iter()
            .any(|r| r.payload.action == "rotate" && r.payload.result == "failed"));
    }

    #[test]
    fn lock_contention_fails_fast_and_recovers() {
        let (_d, path) = tmp_ks();
        let mut ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let mut lock_os = path.as_os_str().to_owned();
        lock_os.push(".lock");
        let lock_path = PathBuf::from(lock_os);
        std::fs::write(&lock_path, "held by another process").unwrap();

        let err = KeystoreLock::acquire_within(&path, Duration::from_millis(100)).unwrap_err();
        assert!(matches!(err, KeyStoreError::LockBusy(_)));

        std::fs::remove_file(&lock_path).unwrap();
        // lock released → mutation works again
        ks.rotate(now_sec(), &ctx()).unwrap();
    }

    // ── PR #9: backup integrity / restore safety ─────────────────────────

    fn tamper_backup(backup: &Path, mutate: impl FnOnce(&mut serde_json::Value)) {
        let mut v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(backup).unwrap()).unwrap();
        mutate(&mut v);
        std::fs::write(backup, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    }

    #[test]
    fn backup_checksum_mismatch_is_rejected_without_touching_live_keystore() {
        let (_d, path) = tmp_ks();
        let ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let before = std::fs::read(&path).unwrap();
        let backup = _d.path().join("ks.backup.json");
        ks.write_backup(&backup, &ctx()).unwrap();

        // Content edit that still parses: checksum must catch it.
        tamper_backup(&backup, |v| {
            v["keystore"]["keys"][0]["created_at"] = serde_json::json!(1);
        });
        let err = KeyStore::restore_from_backup(&backup, &path, 100, &ctx()).unwrap_err();
        assert!(matches!(err, KeyStoreError::BackupIntegrity(_)));
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn backup_with_unsupported_envelope_version_is_rejected() {
        let (_d, path) = tmp_ks();
        let ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let backup = _d.path().join("ks.backup.json");
        ks.write_backup(&backup, &ctx()).unwrap();
        tamper_backup(&backup, |v| {
            v["backup_format_version"] = serde_json::json!(99);
        });
        let err = KeyStore::restore_from_backup(&backup, &path, 100, &ctx()).unwrap_err();
        assert!(matches!(err, KeyStoreError::BackupVersion(_)));
    }

    #[test]
    fn legacy_bare_keystore_backup_is_rejected() {
        let (_d, path) = tmp_ks();
        let ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let backup = _d.path().join("bare.backup.json");
        std::fs::write(&backup, serde_json::to_string_pretty(ks.data()).unwrap()).unwrap();
        let err = KeyStore::restore_from_backup(&backup, &path, 100, &ctx()).unwrap_err();
        assert!(matches!(err, KeyStoreError::BackupVersion(_)));
        assert!(err.to_string().contains("legacy"));
    }

    #[test]
    fn failed_restore_rolls_back_and_keeps_old_tokens_verifiable() {
        let (_d, path) = tmp_ks();
        let mut ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let original = ks.active().clone();
        let original_token = sign_with(&original.private_pem, &original.kid);
        let backup = _d.path().join("ks.backup.json");
        ks.write_backup(&backup, &ctx()).unwrap();

        // live state moves on (rotation) before the restore is attempted
        let (rotated_kid, _) = ks.rotate(now_sec(), &ctx()).unwrap();
        let live_before_restore = std::fs::read(&path).unwrap();

        let err =
            KeyStore::restore_from_backup_with_verification_failure(&backup, &path, 100, &ctx())
                .unwrap_err();
        assert!(matches!(err, KeyStoreError::RestoreFailed(_)));

        // Live keystore is byte-identical to the pre-restore state.
        assert_eq!(std::fs::read(&path).unwrap(), live_before_restore);
        let on_disk = KeyStore::load(&path, 100).unwrap();
        assert_eq!(on_disk.active().kid, rotated_kid);
        // ...so the pre-restore (rotated) world still verifies its tokens,
        // including the original one via the retiring key.
        assert!(on_disk
            .verify_public_pems()
            .iter()
            .any(|p| verify_with(p, &original_token)));

        // Pre-restore snapshot written and the failure audited.
        let snapshots = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("pre-restore"))
            .count();
        assert_eq!(snapshots, 1);
        let records = AuditLedger::beside_keystore(&path).read_all().unwrap();
        assert!(
            records
                .iter()
                .any(|r| r.event_type == "token.jwt_key.restore_failed"
                    && r.payload.result == "failed"),
            "restore failure must be audited, records = {records:?}"
        );
    }

    #[test]
    fn successful_restore_snapshots_previous_state_and_verifies_old_tokens() {
        let (_d, path) = tmp_ks();
        let mut ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let original = ks.active().clone();
        let original_token = sign_with(&original.private_pem, &original.kid);
        let backup = _d.path().join("ks.backup.json");
        ks.write_backup(&backup, &ctx()).unwrap();

        let (rotated_kid, _) = ks.rotate(now_sec(), &ctx()).unwrap();
        let rotated_state = std::fs::read(&path).unwrap();

        let restored = KeyStore::restore_from_backup(&backup, &path, 100, &ctx()).unwrap();
        assert_eq!(restored.active().kid, original.kid);
        // Backup contains only the original key → rotated live state is gone,
        // which is exactly why the pre-restore snapshot must exist.
        assert!(restored.data().keys.iter().all(|k| k.kid != rotated_kid));
        let snapshots: Vec<PathBuf> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.to_string_lossy().contains("pre-restore"))
            .collect();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(std::fs::read(&snapshots[0]).unwrap(), rotated_state);

        // History tokens verify again after the restore + reload.
        let rebooted = KeyStore::init(&path, "np", "nq", 100).unwrap();
        assert!(verify_with(&rebooted.active().public_pem, &original_token));
        let records = AuditLedger::beside_keystore(&path).read_all().unwrap();
        assert!(records.iter().any(|r| r.payload.action == "restore"
            && r.payload.result == "success"
            && r.payload.kid.as_deref() == Some(original.kid.as_str())));
    }

    #[test]
    fn backup_records_audit_without_key_material() {
        let (_d, path) = tmp_ks();
        let ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let backup = _d.path().join("ks.backup.json");
        ks.write_backup(&backup, &ctx()).unwrap();
        let raw = std::fs::read_to_string(AuditLedger::beside_keystore(&path).path()).unwrap();
        assert!(raw.contains("token.jwt_key.backup"));
        assert!(!raw.contains("BEGIN"));
    }
}
