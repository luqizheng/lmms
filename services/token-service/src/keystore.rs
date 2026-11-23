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
//! - writes are atomic (unique tmp file + fsync + rename) and 0600 (unix);
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
//! - backups run under the same cross-process lock and snapshot the
//!   **on-disk** state (reloaded and validated under the lock), wrapped in a
//!   versioned envelope with a SHA-256 integrity checksum; restore validates
//!   everything, snapshots the live keystore, writes under the lock and
//!   verifies the result from disk by full checksum (rolling back on
//!   failure);
//! - lock ownership: the holder keeps the lock file open with an exclusive
//!   kernel lock (flock on Unix / exclusive share mode on Windows), Drop
//!   unlinks only if the recorded owner token still matches, and takeover
//!   requires acquiring that kernel lock — a live holder is never reclaimed
//!   and a stale holder can never delete a newer owner's lock file.

use crate::audit::{AuditAction, AuditContext, AuditLedger, AuditResult};
use jwt_simple::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;
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
/// On-disk format version of the lock file payload. Lock files written by
/// older builds (`pid=<n> at=<ts>` plain text) carry no parseable owner and
/// are taken over only when the kernel lock proves no live holder.
const LOCK_FORMAT_VERSION: u32 = 1;

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

/// Non-secret result of a successful [`KeyStore::write_backup`] (no key
/// material — safe to print in `keytool` output or audit records).
#[derive(Debug, Clone)]
pub struct BackupSummary {
    pub active_kid: String,
    pub keys: usize,
    pub keystore_sha256: String,
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
    ///
    /// Concurrency (PR #9 review F4): the whole operation runs under the
    /// same cross-process lock as rotate/restore, and the snapshot is taken
    /// **from disk while the lock is held** — if another process rotated the
    /// keystore since this instance was loaded, the fresher disk state is
    /// reloaded and validated first, so the backup can never be a torn or
    /// stale snapshot.
    pub fn write_backup(
        &self,
        backup_path: &Path,
        ctx: &AuditContext,
    ) -> Result<BackupSummary, KeyStoreError> {
        match self.write_backup_inner(backup_path) {
            Ok(summary) => {
                self.audit.record(
                    ctx,
                    AuditAction::Backup,
                    AuditResult::Success,
                    Some(&summary.active_kid),
                    None,
                    Some(&format!(
                        "wrote backup ({} keys) to {}",
                        summary.keys,
                        backup_path.display()
                    )),
                );
                Ok(summary)
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

    fn write_backup_inner(&self, backup_path: &Path) -> Result<BackupSummary, KeyStoreError> {
        // Serialise against every other keystore mutation (rotate / restore /
        // other backups). Holding the lock for the whole read-snapshot-write
        // sequence is what makes the backup a consistent point-in-time copy.
        let _lock = KeystoreLock::acquire(&self.path)?;
        // The in-memory `self.data` may be stale if another process wrote the
        // keystore after this instance loaded it. Reload from disk under the
        // lock so the backup reflects the real current state; only fall back
        // to the in-memory snapshot when the file does not exist (first boot).
        let data = match Self::load(&self.path, self.min_retention_sec) {
            Ok(fresh) => fresh.data,
            Err(e) if disk_keystore_missing(&self.path) => {
                tracing::warn!(
                    path = %self.path.display(),
                    error = %e,
                    "keystore file not present on disk during backup; backing up the in-memory snapshot"
                );
                self.data.clone()
            }
            // A corrupt/unreadable keystore must fail the backup, not produce
            // a backup from stale memory that hides the corruption.
            Err(e) => return Err(KeyStoreError::Save(format!(
                "backup aborted: keystore on disk is unreadable ({}); refusing to back up the stale in-memory snapshot: {e}",
                self.path.display()
            ))),
        };
        let checksum = keystore_checksum(&data)?;
        let envelope = BackupEnvelope {
            backup_format_version: BACKUP_FORMAT_VERSION,
            created_at: now_sec(),
            producer: "token-service".to_string(),
            keystore_sha256: checksum.clone(),
            keystore: data,
        };
        let json = serde_json::to_string_pretty(&envelope)
            .map_err(|e| KeyStoreError::Save(format!("serialize backup envelope: {e}")))?;
        atomic_write(backup_path, &json)?;
        tracing::info!(
            path = %backup_path.display(),
            keys = envelope.keystore.keys.len(),
            "wrote keystore backup (versioned envelope with SHA-256 integrity checksum)"
        );
        Ok(BackupSummary {
            active_kid: envelope.keystore.active_kid.clone(),
            keys: envelope.keystore.keys.len(),
            keystore_sha256: checksum,
        })
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
        // back is not a restore. Verification is full-checksum: the
        // reloaded keystore must serialize to exactly the checksum recorded
        // in the backup envelope (active kid / key count alone could miss a
        // mutated key entry).
        let expected_checksum = envelope.keystore_sha256.clone();
        let verification = match Self::load(keystore_path, min_retention_sec) {
            Ok(loaded)
                if loaded.data.active_kid == ks.data.active_kid
                    && loaded.data.keys.len() == ks.data.keys.len() =>
            {
                match keystore_checksum(&loaded.data) {
                    Ok(c) if c == expected_checksum => Ok(()),
                    Ok(c) => Err(KeyStoreError::RestoreFailed(format!(
                        "post-restore checksum mismatch: expected {expected_checksum}, read back {c}"
                    ))),
                    Err(e) => Err(KeyStoreError::RestoreFailed(format!(
                        "post-restore checksum computation failed: {e}"
                    ))),
                }
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
            } else {
                // There was no pre-restore keystore (the file did not
                // exist), so there is nothing to roll back to: the restored
                // (checksum-verified) state stays on disk. This must be
                // impossible to miss: the operator has to know the live
                // keystore now reflects the backup despite the failure.
                tracing::error!(
                    error = %e,
                    path = %keystore_path.display(),
                    "restore verification failed and there is NO pre-restore snapshot to roll back to; \
                     the keystore is left in the restored state — verify it manually or restore another backup"
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

/// True only when the keystore is genuinely absent from disk. Callers hold
/// the cross-process lock, so `exists()` reflects the state the (failed)
/// load saw: absent file ⇒ the load error was NotFound (first-boot path),
/// while a corrupt/unreadable keystore leaves the file on disk and must
/// fail the operation instead of falling back to an in-memory snapshot.
fn disk_keystore_missing(path: &Path) -> bool {
    !path.exists()
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
/// Ownership model (PR #9 review F1/F2 — replaces the old
/// create/unconditional-remove-on-drop scheme that could let a stale process
/// delete a newer process' lock and could reclaim a live lock by mtime):
///
/// - The lock file payload is a JSON [`LockContent`] with a random
///   `owner` token (UUID v4), the creating `pid`, `created_at` and a
///   `format_version`. No secrets.
/// - Mutual exclusion is enforced by the **kernel**, not by file presence:
///   the owner keeps the file handle open for the whole lock lifetime with
///   an exclusive kernel lock — `flock(LOCK_EX)` on Unix,
///   `share_mode(0)` (no sharing allowed) on Windows. A second opener fails
///   while the holder lives, on both platforms.
/// - Takeover of an existing lock file is allowed **only** when that
///   exclusive kernel lock can be acquired, which proves the previous
///   holder is gone (the kernel releases the lock on process exit/crash).
///   This is immune to PID reuse: liveness is decided by the kernel lock,
///   never by mtime or by whether a pid happens to exist. If the kernel
///   lock cannot be acquired (live holder, unexpected error, exotic
///   filesystem) the candidate **fails closed**: it keeps waiting until
///   [`LOCK_WAIT`] and then returns [`KeyStoreError::LockBusy`] — it never
///   deletes or overwrites a lock it does not own.
/// - Drop re-reads the lock file and unlinks it **only** when the recorded
///   owner token still matches its own, so a process that was preempted,
///   taken over and left behind can never delete the new owner's lock. On
///   Unix the unlink happens while the flock is still held, closing the
///   read→remove race window; on Windows the handle must be closed before
///   the unlink (no delete-while-open without FILE_SHARE_DELETE), so the
///   owner token is re-checked after the close.
/// - Takeovers log the reason, the previous holder's pid (best effort) and
///   the lock age — no key material, ever.
#[derive(Debug)]
struct KeystoreLock {
    path: PathBuf,
    owner: String,
    /// Held open for the whole lock lifetime: this handle *is* the kernel
    /// lock (flock on Unix / exclusive share mode on Windows). `Option` so
    /// `Drop` controls the release order explicitly.
    file: Option<std::fs::File>,
}

/// Payload persisted inside the lock file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockContent {
    format_version: u32,
    /// UUID v4 — identifies *this* acquisition, not the process.
    owner: String,
    pid: u32,
    /// Unix seconds when this lock was (re)written by its owner.
    created_at: i64,
}

/// Read + parse the lock file payload; `None` = missing, unreadable or not
/// valid JSON of the expected shape (e.g. plain-text locks from older
/// builds). Callers must treat `None` as "ownership unknown".
fn read_lock_content(path: &Path) -> Option<LockContent> {
    let bytes = std::fs::read(path).ok()?;
    let content: LockContent = serde_json::from_slice(&bytes).ok()?;
    if content.owner.is_empty() {
        return None;
    }
    Some(content)
}

/// Same as [`read_lock_content`] but through an already-held handle —
/// required on Windows, where the holder's `share_mode(0)` handle makes a
/// second open of the path fail.
fn read_lock_content_from(mut file: &std::fs::File) -> Option<LockContent> {
    use std::io::{Read, Seek, SeekFrom};
    file.seek(SeekFrom::Start(0)).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    let content: LockContent = serde_json::from_slice(&buf).ok()?;
    if content.owner.is_empty() {
        return None;
    }
    Some(content)
}

/// Exclusive kernel lock on the already-open handle; `false` = someone else
/// holds it (or the platform cannot tell — treated the same, fail-closed).
#[cfg(unix)]
fn try_lock_kernel_exclusive(file: &std::fs::File) -> bool {
    use std::ffi::c_int;
    use std::os::unix::io::AsRawFd;
    // LOCK_EX | LOCK_NB. Linked from libc, which std links on every unix
    // target (glibc and musl); declared here to avoid a libc crate dep.
    extern "C" {
        fn flock(fd: c_int, operation: c_int) -> c_int;
    }
    const LOCK_EX: c_int = 2;
    const LOCK_NB: c_int = 4;
    let rc = unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) };
    if rc == 0 {
        return true;
    }
    let err = std::io::Error::last_os_error();
    match err.raw_os_error() {
        // EWOULDBLOCK/EAGAIN (11) = held by a live process; EINTR (4) = retry
        // treats it as busy too. Anything else (EINVAL/EOPNOTSUPP/ENOLCK on
        // exotic filesystems) is "cannot determine" → fail-closed.
        Some(11) | Some(4) => false,
        other => {
            tracing::warn!(
                error = %err,
                raw = ?other,
                "flock() failed unexpectedly on the keystore lock file; refusing to take it over (fail-closed)"
            );
            false
        }
    }
}

/// Open options that (on Windows) refuse to share the file with anyone —
/// the Windows counterpart of `flock(LOCK_EX)`: while the holder's handle
/// is open, every other open fails with a sharing violation; when the
/// holder dies, the OS closes the handle and the file becomes openable.
fn exclusive_open_options() -> std::fs::OpenOptions {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        opts.share_mode(0);
    }
    opts
}

/// Outcome of one non-blocking acquisition attempt.
enum TryAcquire {
    Acquired {
        owner: String,
        file: std::fs::File,
    },
    /// Lock is (possibly) held by a live operation: back off and retry.
    Busy,
    /// Unrecoverable IO error.
    Failed(KeyStoreError),
}

fn lock_path_for(keystore_path: &Path) -> PathBuf {
    let mut lock_os = keystore_path.as_os_str().to_owned();
    lock_os.push(".lock");
    PathBuf::from(lock_os)
}

impl KeystoreLock {
    fn acquire(keystore_path: &Path) -> Result<Self, KeyStoreError> {
        Self::acquire_within(keystore_path, LOCK_WAIT)
    }

    fn acquire_within(keystore_path: &Path, wait: Duration) -> Result<Self, KeyStoreError> {
        let path = lock_path_for(keystore_path);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    KeyStoreError::Save(format!("create dir {}: {e}", parent.display()))
                })?;
            }
        }
        let started = std::time::Instant::now();
        loop {
            match Self::try_acquire(&path) {
                TryAcquire::Acquired { owner, file } => {
                    return Ok(Self {
                        path,
                        owner,
                        file: Some(file),
                    });
                }
                TryAcquire::Busy => {
                    if started.elapsed() >= wait {
                        return Err(KeyStoreError::LockBusy(format!(
                            "{} held by a live key operation (kernel lock busy; waited {}ms); \
                             retry after it finishes — the lock of a live holder is never reclaimed",
                            path.display(),
                            wait.as_millis()
                        )));
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                TryAcquire::Failed(e) => return Err(e),
            }
        }
    }

    /// One non-blocking attempt:
    /// 1. `create_new` — atomic on every supported filesystem — wins the
    ///    free lock; on Unix the exclusive flock is taken *before* the file
    ///    counts as held, so a contender racing in between cannot end up
    ///    with two simultaneous owners (worst case we lose our creation and
    ///    retry);
    /// 2. on `AlreadyExists`, try to acquire the exclusive kernel lock on
    ///    the existing file: success proves the previous holder is dead and
    ///    the lock is safe to take over; any failure = live holder or
    ///    unknown state → [`TryAcquire::Busy`] (fail-closed, never delete).
    fn try_acquire(path: &Path) -> TryAcquire {
        let owner = uuid::Uuid::new_v4().to_string();
        // Fresh creation path. On unix the file is created 0600 outright
        // (mode in OpenOptions) so no key-adjacent file ever exists with
        // group/other read. On Windows share_mode(0) makes the open itself
        // exclusive from the instant the file is created.
        let mut create = exclusive_open_options();
        create.create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            create.mode(0o600);
        }
        match create.open(path) {
            Ok(file) => {
                // Close the create↔flock race on Unix: we only own the lock
                // once the kernel lock is ours. Losing the race is benign —
                // the winner rewrote the payload and we simply retry.
                #[cfg(unix)]
                if !try_lock_kernel_exclusive(&file) {
                    return TryAcquire::Busy;
                }
                let content = LockContent {
                    format_version: LOCK_FORMAT_VERSION,
                    owner: owner.clone(),
                    pid: std::process::id(),
                    created_at: now_sec(),
                };
                if let Err(e) = write_lock_content(&file, &content) {
                    // The lock file exists but we never became the owner:
                    // release our kernel lock; the next contender will take
                    // over via the (now unheld) kernel lock.
                    return TryAcquire::Failed(KeyStoreError::Save(format!(
                        "write lock content {}: {e}",
                        path.display()
                    )));
                }
                tracing::debug!(path = %path.display(), "acquired keystore lock (created)");
                return TryAcquire::Acquired { owner, file };
            }
            Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => {
                return TryAcquire::Failed(KeyStoreError::Save(format!(
                    "create lock {}: {e}",
                    path.display()
                )));
            }
            Err(_) => {} // exists → takeover path below
        }

        // Takeover path: acquiring the exclusive kernel lock on the existing
        // file is the *only* proof that the previous holder is gone. On
        // Windows the exclusive open itself is the lock; on unix flock is
        // taken after the open.
        let file = match exclusive_open_options().open(path) {
            Ok(f) => f,
            // Sharing violation (live holder), permission error, anything:
            // cannot prove it is free → fail closed.
            Err(_) => return TryAcquire::Busy,
        };
        #[cfg(unix)]
        if !try_lock_kernel_exclusive(&file) {
            return TryAcquire::Busy;
        }
        // We hold the kernel lock: the previous holder cannot come back
        // (its kernel lock was released on exit/crash and cannot be
        // re-acquired while we hold it). Log what we are replacing. The old
        // payload is read through the handle we hold — on Windows a second
        // open of the path would hit our own share_mode(0) exclusion.
        match read_lock_content_from(&file) {
            Some(old) => {
                let age = now_sec() - old.created_at;
                tracing::info!(
                    path = %path.display(),
                    previous_pid = old.pid,
                    previous_created_at = old.created_at,
                    lock_age_sec = age,
                    "taking over keystore lock: previous holder released the kernel lock (crashed or exited)"
                );
            }
            None => {
                tracing::info!(
                    path = %path.display(),
                    "taking over keystore lock: no kernel holder and unreadable/legacy lock content"
                );
            }
        }
        let content = LockContent {
            format_version: LOCK_FORMAT_VERSION,
            owner: owner.clone(),
            pid: std::process::id(),
            created_at: now_sec(),
        };
        if let Err(e) = write_lock_content(&file, &content) {
            return TryAcquire::Failed(KeyStoreError::Save(format!(
                "rewrite lock content on takeover {}: {e}",
                path.display()
            )));
        }
        tracing::debug!(path = %path.display(), "acquired keystore lock (takeover)");
        TryAcquire::Acquired { owner, file }
    }
}

/// (Over)write the lock payload through the already-locked handle: truncate
/// to zero, write the JSON, fsync. The handle is exclusively ours at this
/// point, so no other process can interleave.
fn write_lock_content(mut file: &std::fs::File, content: &LockContent) -> std::io::Result<()> {
    use std::io::{Seek, SeekFrom, Write};
    let json = serde_json::to_string(content).map_err(std::io::Error::other)?;
    file.seek(SeekFrom::Start(0))?;
    file.set_len(0)?;
    file.write_all(json.as_bytes())?;
    file.sync_all()
}

impl Drop for KeystoreLock {
    fn drop(&mut self) {
        // Close the kernel lock first on Windows: a file opened with
        // share_mode(0) cannot be unlinked while our handle is open, and
        // the handle must go before remove_file. On Unix the unlink runs
        // while the flock is still held, so no contender can take over in
        // between (their flock/open attempts fail until we close).
        #[cfg(windows)]
        {
            self.file.take(); // release the exclusive handle
        }
        // Ownership re-check before deleting (F1): only remove when the
        // file still records OUR owner token. Missing / unreadable /
        // foreign-owned → leave it alone, never panic.
        let owned = read_lock_content(&self.path)
            .map(|c| c.owner == self.owner)
            .unwrap_or(false);
        if owned {
            if let Err(e) = std::fs::remove_file(&self.path) {
                // Best-effort cleanup: a leftover unowned lock is taken over
                // safely by the next acquire; never panic from Drop.
                tracing::warn!(
                    path = %self.path.display(),
                    error = %e,
                    "failed to remove the keystore lock file on drop"
                );
            }
        } else {
            tracing::warn!(
                path = %self.path.display(),
                "keystore lock file no longer owned by this handle at drop; leaving it in place"
            );
        }
        // Release the flock after the unlink on Unix.
        #[cfg(unix)]
        {
            self.file.take();
        }
    }
}

/// Write `contents` to `path` atomically: unique-named tmp file in the same
/// directory, fsync, rename over the target. Unix: file created 0600 (never
/// a 0644 window), directory 0700 (skipped for bare relative paths —
/// chmod'ing "." would tighten an unrelated working directory).
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
    if parent != Path::new(".") {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| KeyStoreError::Save(format!("chmod dir: {e}")))?;
        }
    }
    // Unique tmp name: all writers are serialised by the keystore lock, but
    // a crashed run (or a tmp from an older build) must never be able to
    // clobber a concurrent write through a shared fixed name.
    let tmp = path.with_file_name(format!(
        "{}.tmp.{}",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("keystore"),
        uuid::Uuid::new_v4()
    ));
    let write_result = (|| {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts
            .open(&tmp)
            .map_err(|e| KeyStoreError::Save(format!("create {}: {e}", tmp.display())))?;
        f.write_all(contents)
            .map_err(|e| KeyStoreError::Save(format!("write {}: {e}", tmp.display())))?;
        f.sync_all()
            .map_err(|e| KeyStoreError::Save(format!("fsync {}: {e}", tmp.display())))?;
        drop(f);
        std::fs::rename(&tmp, path).map_err(|e| {
            KeyStoreError::Save(format!(
                "rename {} -> {}: {e}",
                tmp.display(),
                path.display()
            ))
        })?;
        Ok(())
    })();
    if write_result.is_err() {
        // Never leave our tmp behind to confuse the next run.
        let _ = std::fs::remove_file(&tmp);
    }
    write_result
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

    // ── PR #9 P2 F1: lock ownership ──────────────────────────────────────
    //
    // Platform note for these tests: on Windows the holder's share_mode(0)
    // handle blocks *any* other open of the lock file (reads and writes of
    // the payload included), so tampering while held is impossible there and
    // is asserted as such. On Unix flock is advisory: the payload CAN be
    // rewritten behind a live holder's back, which is exactly what the Drop
    // ownership re-check defends against.

    #[test]
    fn lock_drop_removes_own_lock() {
        let (_d, path) = tmp_ks();
        let lock_path = lock_path_for(&path);
        // Payload content checks need path access, which on Windows requires
        // the exclusive handle (holder-only) and on Unix works while held —
        // so they run inside the held scope, unix-only; after drop the file
        // is gone on every platform.
        #[cfg(unix)]
        {
            let lock = KeystoreLock::acquire(&path).unwrap();
            assert!(lock_path.exists());
            let content = read_lock_content(&lock_path).expect("own lock readable");
            assert_eq!(content.owner, lock.owner);
            assert_eq!(content.pid, std::process::id());
            assert_eq!(content.format_version, LOCK_FORMAT_VERSION);
            drop(lock);
            assert!(!lock_path.exists(), "own lock must be removed on drop");
        }
        // Windows: behavioral equivalents — exclusive hold, clean release,
        // fresh ownership token on every acquisition.
        #[cfg(windows)]
        {
            let first = {
                let lock = KeystoreLock::acquire(&path).unwrap();
                assert!(lock_path.exists());
                lock.owner.clone()
            };
            assert!(!lock_path.exists(), "own lock must be removed on drop");
            let second = {
                let lock = KeystoreLock::acquire(&path).unwrap();
                lock.owner.clone()
            };
            assert_ne!(second, first, "each acquisition gets a fresh owner token");
            assert!(!lock_path.exists());
        }
    }

    #[test]
    fn lock_drop_does_not_remove_foreign_owner_lock() {
        let (_d, path) = tmp_ks();
        let lock_path = lock_path_for(&path);
        let foreign = serde_json::to_vec(&LockContent {
            format_version: LOCK_FORMAT_VERSION,
            owner: "foreign-owner".into(),
            pid: 999_999,
            created_at: now_sec(),
        })
        .unwrap();
        #[cfg(unix)]
        {
            // flock is advisory: a takeover may have rewritten the payload
            // while this holder was paused. The Drop ownership re-check
            // must keep the new owner's lock file intact.
            let lock = KeystoreLock::acquire(&path).unwrap();
            std::fs::write(&lock_path, &foreign).unwrap();
            drop(lock);
            let after = read_lock_content(&lock_path)
                .expect("foreign lock must survive the old owner's drop");
            assert_eq!(
                after.owner, "foreign-owner",
                "a stale owner must never delete the new owner's lock"
            );
        }
        #[cfg(windows)]
        {
            let lock = KeystoreLock::acquire(&path).unwrap();
            // The exclusive handle blocks every external rewrite: the
            // scenario cannot occur, and that refusal IS the protection.
            assert!(
                std::fs::write(&lock_path, &foreign).is_err(),
                "external payload rewrite must fail while the lock is held"
            );
            drop(lock);
            assert!(!lock_path.exists(), "our own unmodified lock is removed");
        }
    }

    #[test]
    fn concurrent_owner_replacement_never_lets_old_owner_delete_new_lock() {
        // Deterministic interleaving via channels (no sleeps): a second
        // "owner" replaces the payload after the main thread signals, then
        // the main thread drops its lock.
        let (_d, path) = tmp_ks();
        let lock_path = lock_path_for(&path);
        let stale_lock = KeystoreLock::acquire(&path).unwrap();

        let (go_tx, go_rx) = std::sync::mpsc::channel::<()>();
        let (done_tx, done_rx) = std::sync::mpsc::channel::<bool>();
        let replacer_path = lock_path.clone();
        let replacer = std::thread::spawn(move || {
            go_rx.recv().expect("go signal from main");
            let new_owner = serde_json::to_vec(&LockContent {
                format_version: LOCK_FORMAT_VERSION,
                owner: "new-owner".into(),
                pid: 1,
                created_at: now_sec(),
            })
            .unwrap();
            let ok = std::fs::write(&replacer_path, &new_owner).is_ok();
            done_tx.send(ok).unwrap();
        });
        go_tx.send(()).unwrap();
        let replaced = done_rx.recv().unwrap();
        drop(stale_lock);
        replacer.join().unwrap();

        #[cfg(unix)]
        {
            assert!(replaced, "unix payload rewrite must succeed");
            let after = read_lock_content(&lock_path).unwrap();
            assert_eq!(
                after.owner, "new-owner",
                "old owner's drop must not touch the new lock"
            );
        }
        #[cfg(windows)]
        {
            assert!(!replaced, "windows rewrite must be refused while held");
            assert!(
                !lock_path.exists(),
                "unmodified own lock is removed on drop"
            );
        }
        // And the next acquirer takes the lock cleanly either way; its
        // payload is verified inside its held scope (unix) or behaviorally
        // (windows: successful exclusive hold ⇒ fresh payload).
        {
            let next = KeystoreLock::acquire(&path).unwrap();
            #[cfg(unix)]
            assert_eq!(read_lock_content(&lock_path).unwrap().owner, next.owner);
            drop(next);
            assert!(!lock_path.exists());
        }
    }

    #[test]
    #[cfg(unix)]
    fn lock_drop_tolerates_corrupt_or_missing_lock_file() {
        let (_d, path) = tmp_ks();
        let lock_path = lock_path_for(&path);
        // Corrupt payload at drop time: no panic, no deletion.
        let l1 = KeystoreLock::acquire(&path).unwrap();
        std::fs::write(&lock_path, b"not json at all").unwrap();
        drop(l1);
        assert!(lock_path.exists(), "unparseable lock is left in place");
        // Missing file at drop time: no panic either.
        let l2 = KeystoreLock::acquire(&path).unwrap();
        std::fs::remove_file(&lock_path).unwrap();
        drop(l2);
        assert!(!lock_path.exists());
    }

    #[test]
    #[cfg(windows)]
    fn lock_file_is_exclusive_while_held_on_windows() {
        let (_d, path) = tmp_ks();
        let lock_path = lock_path_for(&path);
        let lock = KeystoreLock::acquire(&path).unwrap();
        // share_mode(0): rewrite AND delete by any other open fail while
        // the holder lives — the corruption scenarios cannot even arise.
        assert!(std::fs::write(&lock_path, b"tampered").is_err());
        assert!(std::fs::remove_file(&lock_path).is_err());
        drop(lock);
        assert!(!lock_path.exists());
    }

    // ── PR #9 P2 F2: takeover policy (kernel lock, fail-closed) ─────────

    #[test]
    fn live_holder_lock_is_never_reclaimed_even_when_aged() {
        let (_d, path) = tmp_ks();
        #[cfg(unix)]
        let lock_path = lock_path_for(&path);
        let lock = KeystoreLock::acquire(&path).unwrap();
        let holder_owner = lock.owner.clone();
        // Forge a day-old creation timestamp (unix: flock is advisory so the
        // payload is writable behind the holder's back): any mtime/age-based
        // reclaimer would call this stale. The kernel lock is live, so the
        // lock must NOT be reclaimed — acquisition fails closed.
        #[cfg(unix)]
        {
            let aged = serde_json::to_vec(&LockContent {
                format_version: LOCK_FORMAT_VERSION,
                owner: holder_owner.clone(),
                pid: std::process::id(),
                created_at: now_sec() - 86_400,
            })
            .unwrap();
            std::fs::write(&lock_path, &aged).unwrap();
        }
        let err = KeystoreLock::acquire_within(&path, Duration::from_millis(150)).unwrap_err();
        assert!(matches!(err, KeyStoreError::LockBusy(_)));
        drop(lock);
        // Released → immediately acquirable, with fresh ownership (payload
        // verified inside the new holder's scope, unix-only).
        let next = KeystoreLock::acquire(&path).unwrap();
        #[cfg(unix)]
        {
            let content = read_lock_content(&lock_path).unwrap();
            assert_ne!(
                content.owner, holder_owner,
                "the new acquisition must have rewritten ownership"
            );
        }
        #[cfg(windows)]
        assert_ne!(next.owner, holder_owner, "fresh owner token after release");
        drop(next);
    }

    #[test]
    fn dead_holder_lock_is_taken_over_and_ownership_rewritten() {
        let (_d, path) = tmp_ks();
        let lock_path = lock_path_for(&path);
        std::fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
        // A crashed holder: file on disk, no kernel lock held, dead pid,
        // ancient timestamp → safe takeover (liveness proven by the kernel,
        // immune to pid reuse on both counts).
        std::fs::write(
            &lock_path,
            serde_json::to_vec(&LockContent {
                format_version: LOCK_FORMAT_VERSION,
                owner: "crashed-owner".into(),
                pid: 4_000_000_000, // never a plausible live pid of this test
                created_at: now_sec() - 3_600,
            })
            .unwrap(),
        )
        .unwrap();
        let owner = {
            let lock = KeystoreLock::acquire(&path).unwrap();
            assert_ne!(lock.owner, "crashed-owner");
            // Verify the takeover rewrote the payload, inside the held scope.
            #[cfg(unix)]
            {
                let content = read_lock_content(&lock_path).unwrap();
                assert_eq!(content.owner, lock.owner);
                assert_eq!(content.pid, std::process::id());
                assert_eq!(content.format_version, LOCK_FORMAT_VERSION);
            }
            lock.owner.clone()
        };
        assert!(!lock_path.exists(), "lock removed on release");
        let _ = owner;
    }

    #[test]
    fn legacy_or_corrupt_lock_content_taken_over_only_when_unheld() {
        let (_d, path) = tmp_ks();
        let lock_path = lock_path_for(&path);
        std::fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
        // Old-build plain-text payload with no kernel holder: the payload is
        // unreadable but the kernel lock proves nobody holds the lock, so a
        // takeover is safe and rewrites the content into the new format.
        std::fs::write(&lock_path, "pid=12345 at=1700000000").unwrap();
        {
            let lock = KeystoreLock::acquire_within(&path, Duration::from_millis(300)).unwrap();
            #[cfg(unix)]
            {
                let content = read_lock_content(&lock_path).unwrap();
                assert_eq!(content.owner, lock.owner);
                assert_eq!(content.format_version, LOCK_FORMAT_VERSION);
            }
            #[cfg(windows)]
            assert!(
                !lock.owner.is_empty(),
                "takeover produced a fresh owner token"
            );
        }
        assert!(!lock_path.exists());
    }

    #[test]
    fn lock_contention_fails_fast_and_recovers() {
        let (_d, path) = tmp_ks();
        let mut ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
        // Hold a REAL kernel lock, not just a file on disk.
        let held = KeystoreLock::acquire(&path).unwrap();
        let err = KeystoreLock::acquire_within(&path, Duration::from_millis(150)).unwrap_err();
        assert!(matches!(err, KeyStoreError::LockBusy(_)));
        assert!(
            err.to_string().contains("live"),
            "LockBusy must state the live-holder policy: {err}"
        );
        drop(held);
        // lock released → mutation works again
        ks.rotate(now_sec(), &ctx()).unwrap();
    }

    #[test]
    fn sequential_lock_acquires_never_leak_state() {
        let (_d, path) = tmp_ks();
        let lock_path = lock_path_for(&path);
        for _ in 0..5 {
            let l = KeystoreLock::acquire(&path).unwrap();
            assert!(lock_path.exists());
            drop(l);
            assert!(!lock_path.exists(), "every release removes the lock");
        }
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

    // ── PR #9 P2 F4: backup consistency ──────────────────────────────────

    #[test]
    fn backup_snapshots_fresh_disk_state_not_stale_memory() {
        let (_d, path) = tmp_ks();
        let instance_a = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let mut instance_b = KeyStore::load(&path, 100).unwrap();
        let (new_kid, _) = instance_b.rotate(now_sec(), &ctx()).unwrap();
        // A's memory still holds the pre-rotation snapshot. The backup must
        // reload the disk state under the lock — never back up stale memory.
        let backup = _d.path().join("ks.backup.json");
        let summary = instance_a.write_backup(&backup, &ctx()).unwrap();
        assert_eq!(summary.active_kid, new_kid);
        let env: BackupEnvelope =
            serde_json::from_str(&std::fs::read_to_string(&backup).unwrap()).unwrap();
        assert_eq!(env.keystore.active_kid, new_kid);
        assert_eq!(env.keystore_sha256, summary.keystore_sha256);
        // The recorded checksum matches the keystore that is on disk now.
        let disk = KeyStore::load(&path, 100).unwrap();
        assert_eq!(keystore_checksum(disk.data()).unwrap(), env.keystore_sha256);
        // (silence the unused-binding lint on the stale instance)
        let _ = instance_a;
    }

    #[test]
    fn concurrent_rotate_and_backup_produce_serialized_consistent_backups() {
        // Barrier-synchronised; the assertion is structural, so no sleeps
        // are needed: every backup must be internally checksum-consistent
        // AND its key list must be a prefix of the final on-disk key order.
        // A torn or stale snapshot would violate the prefix property.
        const ROTATIONS: usize = 3;
        const BACKUPS: usize = 3;
        let (_d, path) = tmp_ks();
        let ks0 = KeyStore::init(&path, "np", "nq", 100).unwrap();
        assert_eq!(ks0.data().keys.len(), 1);

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(ROTATIONS + BACKUPS));
        let mut rotators = Vec::new();
        let mut backupers = Vec::new();
        for i in 0..ROTATIONS {
            let barrier = barrier.clone();
            let p = path.clone();
            rotators.push(std::thread::spawn(move || {
                let mut ks = KeyStore::load(&p, 100).unwrap();
                barrier.wait();
                ks.rotate(now_sec(), &ctx())
            }));
            let _ = i;
        }
        for i in 0..BACKUPS {
            let barrier = barrier.clone();
            let p = path.clone();
            let out = _d.path().join(format!("concurrent-backup-{i}.json"));
            backupers.push(std::thread::spawn(move || {
                let ks = KeyStore::load(&p, 100).unwrap();
                barrier.wait();
                ks.write_backup(&out, &ctx())
            }));
        }

        let mut rotations_ok = 0;
        for h in rotators {
            match h.join().unwrap() {
                Ok(_) => rotations_ok += 1,
                // Concurrent rotations from the same base state: exactly the
                // first writer wins, later ones must fail with Stale.
                Err(e) => assert!(
                    matches!(e, KeyStoreError::Stale(_)),
                    "rotation failures must be explicit Stale errors, got: {e}"
                ),
            }
        }
        assert!(rotations_ok >= 1, "at least one rotation must succeed");

        for h in backupers {
            h.join()
                .unwrap()
                .expect("every backup must succeed (lock serialises them)");
        }

        let final_disk = KeyStore::load(&path, 100).unwrap();
        let final_kids: Vec<String> = final_disk
            .data()
            .keys
            .iter()
            .map(|k| k.kid.clone())
            .collect();
        for i in 0..BACKUPS {
            let backup = _d.path().join(format!("concurrent-backup-{i}.json"));
            let env: BackupEnvelope =
                serde_json::from_str(&std::fs::read_to_string(&backup).unwrap()).unwrap();
            assert_eq!(
                keystore_checksum(&env.keystore).unwrap(),
                env.keystore_sha256,
                "backup {i} must be internally consistent"
            );
            let kids: Vec<String> = env.keystore.keys.iter().map(|k| k.kid.clone()).collect();
            assert!(
                final_kids.starts_with(&kids),
                "backup {i} keys {kids:?} must be a serial prefix of the final state {final_kids:?}"
            );
            assert!(
                env.keystore
                    .keys
                    .iter()
                    .any(|k| k.kid == env.keystore.active_kid),
                "backup {i}: active kid must be one of its own keys"
            );
        }
    }

    #[test]
    fn concurrent_backup_and_restore_stay_consistent() {
        // Restore replaces the keystore wholesale with the backup content,
        // so the final on-disk state is deterministic (the restored state)
        // regardless of which thread lands last; the mid-flight backup must
        // still be a checksum-valid serial state.
        let (_d, path) = tmp_ks();
        let mut ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let backup = _d.path().join("ks.backup.json");
        ks.write_backup(&backup, &ctx()).unwrap();
        let original_kid = ks.active().kid.clone();
        let (rotated_kid, _) = ks.rotate(now_sec(), &ctx()).unwrap();

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let b1 = barrier.clone();
        let backup_thread = {
            let p = path.clone();
            let out = _d.path().join("during-restore.backup.json");
            std::thread::spawn(move || {
                let live = KeyStore::load(&p, 100).unwrap();
                b1.wait();
                live.write_backup(&out, &ctx())
            })
        };
        let b2 = barrier;
        let restore_thread = {
            let p = path.clone();
            let src = backup.clone();
            std::thread::spawn(move || {
                b2.wait();
                KeyStore::restore_from_backup(&src, &p, 100, &ctx())
            })
        };
        let mid_backup = backup_thread.join().unwrap().expect("backup succeeds");
        restore_thread.join().unwrap().expect("restore succeeds");

        let env: BackupEnvelope = serde_json::from_str(
            &std::fs::read_to_string(_d.path().join("during-restore.backup.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            keystore_checksum(&env.keystore).unwrap(),
            env.keystore_sha256,
            "mid-flight backup must be internally consistent"
        );
        assert!(
            mid_backup.active_kid == original_kid || mid_backup.active_kid == rotated_kid,
            "mid-flight backup must be a serial state, got {}",
            mid_backup.active_kid
        );
        // The restore always owns the final disk state.
        let final_disk = KeyStore::load(&path, 100).unwrap();
        assert_eq!(final_disk.active().kid, original_kid);
    }

    #[test]
    fn concurrent_backups_to_same_target_never_corrupt_each_other() {
        const BACKUPS: usize = 4;
        let (_d, path) = tmp_ks();
        KeyStore::init(&path, "np", "nq", 100).unwrap();
        let out = _d.path().join("same-target.backup.json");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(BACKUPS));
        let mut handles = Vec::new();
        for _ in 0..BACKUPS {
            let barrier = barrier.clone();
            let p = path.clone();
            let out = out.clone();
            handles.push(std::thread::spawn(move || {
                let ks = KeyStore::load(&p, 100).unwrap();
                barrier.wait();
                ks.write_backup(&out, &ctx())
            }));
        }
        for h in handles {
            h.join().unwrap().expect("each backup succeeds");
        }
        // The final file is a complete, valid envelope (last writer wins).
        let env: BackupEnvelope =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(
            keystore_checksum(&env.keystore).unwrap(),
            env.keystore_sha256
        );
        // Unique tmp names leave nothing behind.
        let tmp_leftovers = std::fs::read_dir(_d.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
            .count();
        assert_eq!(
            tmp_leftovers, 0,
            "failed/finished backups must not leave tmp files"
        );
    }

    #[test]
    fn failed_backup_does_not_damage_existing_backup() {
        let (_d, path) = tmp_ks();
        let ks = KeyStore::init(&path, "np", "nq", 100).unwrap();
        let good = _d.path().join("good.backup.json");
        ks.write_backup(&good, &ctx()).unwrap();
        let before = std::fs::read(&good).unwrap();

        // Target whose parent chain goes through a regular file: the write
        // fails before anything is renamed, and the good backup is intact.
        let blocker = _d.path().join("blocker-file");
        std::fs::write(&blocker, b"i am a file").unwrap();
        let bad_target = blocker.join("nested").join("bad.backup.json");
        let err = ks.write_backup(&bad_target, &ctx()).unwrap_err();
        assert!(matches!(err, KeyStoreError::Save(_)), "got: {err}");
        assert_eq!(std::fs::read(&good).unwrap(), before);
        assert!(
            !std::fs::read_dir(_d.path())
                .unwrap()
                .filter_map(|e| e.ok())
                .any(|e| e.file_name().to_string_lossy().contains(".tmp.")),
            "failed backup must clean up its tmp file"
        );
    }
}
