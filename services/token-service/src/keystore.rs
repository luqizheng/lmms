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

use jwt_simple::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// kid assigned to keys imported from the legacy `private.pem`/`public.pem`
/// pair. Historical tokens carry `kid:"default"` in their header (the old
/// hard-coded `JWT_KEY_ID`), so imported keys keep that kid and stay in the
/// JWKS until retired — otherwise every pre-migration token would fail
/// verification with "unknown kid".
pub const LEGACY_KID: &str = "default";
/// On-disk format version. Bumping requires a migration path.
pub const KEYSTORE_VERSION: u32 = 1;

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

#[derive(Debug)]
pub struct KeyStore {
    path: PathBuf,
    min_retention_sec: i64,
    data: KeyStoreData,
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
                let ks = Self::from_legacy_pems(priv_pem, pub_pem, keystore_path, min_retention_sec)?;
                ks.save()?;
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
        let ks = Self {
            path: path.to_path_buf(),
            min_retention_sec,
            data,
        };
        ks.save()?;
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

    /// Serialization used by backups (pretty JSON, includes private keys —
    /// callers must protect the output file; `keytool backup` writes 0600).
    pub fn backup_json(&self) -> Result<String, KeyStoreError> {
        serde_json::to_string_pretty(&self.data)
            .map_err(|e| KeyStoreError::Save(format!("serialize keystore: {e}")))
    }

    // ─────────────────────────────────────────────────────────────────────
    // Mutations (each one persists atomically before returning)
    // ─────────────────────────────────────────────────────────────────────

    /// Rotate: generate a new key that becomes the only signer; the previous
    /// active key downgrades to `retiring` (verify-only, still in JWKS) so
    /// outstanding tokens keep working. Returns `(new_kid, previous_kid)`.
    pub fn rotate(&mut self, now_sec: i64) -> Result<(String, String), KeyStoreError> {
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
    pub fn retire(&mut self, kid: &str, now_sec: i64) -> Result<(), KeyStoreError> {
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
    pub fn remove(&mut self, kid: &str, now_sec: i64) -> Result<(), KeyStoreError> {
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
    pub fn save(&self) -> Result<(), KeyStoreError> {
        let json = serde_json::to_string_pretty(&self.data)
            .map_err(|e| KeyStoreError::Save(format!("serialize keystore: {e}")))?;
        atomic_write(&self.path, &json)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Backup / restore
    // ─────────────────────────────────────────────────────────────────────

    /// Write a backup file (0600 on unix). The backup contains all private
    /// key material — operators must store it in protected storage and keep
    /// it out of git (`*.backup.json` is gitignored for this reason).
    pub fn write_backup(&self, backup_path: &Path) -> Result<(), KeyStoreError> {
        let json = self.backup_json()?;
        atomic_write(backup_path, &json)?;
        tracing::info!(path = %backup_path.display(), keys = self.data.keys.len(), "wrote keystore backup");
        Ok(())
    }

    /// Restore a backup over the live keystore path: validates the backup
    /// fully (JSON shape, every PEM parses, private/public match) before
    /// any disk write. After restore + restart, previously issued tokens
    /// verify again.
    pub fn restore_from_backup(
        backup_path: &Path,
        keystore_path: &Path,
        min_retention_sec: i64,
    ) -> Result<Self, KeyStoreError> {
        let raw = std::fs::read_to_string(backup_path).map_err(|e| {
            KeyStoreError::Load(format!("read backup {}: {e}", backup_path.display()))
        })?;
        let data: KeyStoreData = serde_json::from_str(&raw).map_err(|e| {
            KeyStoreError::Load(format!("parse backup {}: {e}", backup_path.display()))
        })?;
        validate_data(&data)?;
        let ks = Self {
            path: keystore_path.to_path_buf(),
            min_retention_sec,
            data,
        };
        ks.save()?;
        tracing::info!(
            backup = %backup_path.display(),
            path = %keystore_path.display(),
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

/// Write `contents` to `path` atomically: tmp file in the same directory,
/// fsync, rename over the target. Unix: file 0600, directory 0700.
fn atomic_write(path: &Path, contents: &str) -> Result<(), KeyStoreError> {
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
        f.write_all(contents.as_bytes())
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

        let (new_kid, prev_kid) = ks.rotate(now_sec()).unwrap();
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

        let (new_kid, _prev) = ks.rotate(now_sec()).unwrap();
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
        let (_, prev) = ks.rotate(now_sec()).unwrap();
        assert_eq!(prev, old_kid);

        // active signer cannot be retired or deleted directly
        assert!(ks.retire(&ks.active().kid.clone(), now_sec()).is_err());
        assert!(ks.remove(&ks.active().kid.clone(), now_sec()).is_err());

        // retiring → retired leaves JWKS
        ks.retire(&prev, now_sec()).unwrap();
        assert_eq!(ks.public_entries().len(), 1);
        assert!(ks.public_entries().iter().all(|k| k.kid == ks.active().kid));

        // retention gate blocks deletion, then allows it after retention
        let err = ks.remove(&prev, now_sec()).unwrap_err();
        assert!(err.to_string().contains("retention not satisfied"));
        ks.remove(&prev, now_sec() + 1000).unwrap();
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
        ks.write_backup(&backup_path).unwrap();

        // simulate total loss: keystore gone, fresh empty dir
        std::fs::remove_file(&path).unwrap();
        let restored = KeyStore::restore_from_backup(&backup_path, &path, 100).unwrap();
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
        assert!(KeyStore::restore_from_backup(&bad, &path, 100).is_err());
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
        let (new_kid, _prev) = ks.rotate(now_sec()).unwrap();
        let reloaded = KeyStore::load(&path, 100).unwrap();
        assert_eq!(reloaded.active().kid, new_kid);
        assert_eq!(reloaded.data().keys.len(), 2);
    }
}
