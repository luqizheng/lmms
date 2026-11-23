//! `keytool` — offline operator tool for the token-service JWT keystore.
//!
//! For deployments that prefer stopping the service for key operations, or
//! for backup/restore drills. Online equivalent endpoints live at
//! `/admin/keys*` (service-jwt gated). See
//! `docs/superpowers/specs/2026-11-23-token-jwt-key-persistence-design.md`.
//!
//! Usage:
//!   keytool inspect  --keystore <p>                    # kid/status list, no key material
//!   keytool rotate   --keystore <p>                    # new active key, old → retiring
//!   keytool retire   --keystore <p> --kid <kid>        # retiring → retired (leaves JWKS)
//!   keytool delete   --keystore <p> --kid <kid>        # retired + retention satisfied only
//!   keytool backup   --keystore <p> --out <f>          # 0600 backup file (KEEP OUT OF GIT)
//!   keytool restore  --backup <f> --keystore <p>       # validate-then-atomic-restore
//!
//! Min-retention for `delete` mirrors the service env:
//!   TOKEN_JWT_MIN_RETENTION_SEC (default 2592000 = 30 days).

use std::path::PathBuf;

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn now_sec() -> i64 {
    chrono::Utc::now().timestamp()
}

fn die(msg: &str) -> ! {
    eprintln!("keytool: {msg}");
    std::process::exit(1);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("");

    let min_retention: i64 = std::env::var("TOKEN_JWT_MIN_RETENTION_SEC")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2_592_000);

    match cmd {
        "inspect" => inspect(require(&args, "--keystore")),
        "rotate" => rotate(require(&args, "--keystore")),
        "retire" => retire(require(&args, "--keystore"), &require(&args, "--kid")),
        "delete" => delete(require(&args, "--keystore"), &require(&args, "--kid"), min_retention),
        "backup" => backup(require(&args, "--keystore"), &require(&args, "--out")),
        "restore" => restore(&require(&args, "--backup"), &require(&args, "--keystore"), min_retention),
        _ => die("usage: keytool <inspect|rotate|retire|delete|backup|restore> --keystore <path> [--kid <kid>] [--out <file>] [--backup <file>]"),
    }
}

fn require(args: &[String], flag: &str) -> String {
    arg_value(args, flag).unwrap_or_else(|| die(&format!("missing {flag} <value>")))
}

fn open_keystore(path: &str, min_retention: i64) -> token_service::keystore::KeyStore {
    let ks_path = PathBuf::from(path);
    // Legacy import is a service-startup concern; the tool always loads an
    // existing keystore (fail-closed on corruption).
    token_service::keystore::KeyStore::load(&ks_path, min_retention)
        .unwrap_or_else(|e| die(&format!("load keystore {path}: {e}")))
}

fn min_retention_from_env() -> i64 {
    std::env::var("TOKEN_JWT_MIN_RETENTION_SEC")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2_592_000)
}

fn inspect(path: String) {
    let ks = open_keystore(&path, min_retention_from_env());
    println!("keystore          : {path}");
    println!("version           : {}", ks.data().version);
    println!("active_kid        : {}", ks.active().kid);
    println!("min_retention_sec : {}", ks.min_retention_sec());
    println!("keys ({}):", ks.data().keys.len());
    for k in &ks.data().keys {
        let status = match k.status {
            token_service::keystore::KeyStatus::Active => "active",
            token_service::keystore::KeyStatus::Retiring => "retiring",
            token_service::keystore::KeyStatus::Retired => "retired",
        };
        let retired = k
            .retired_at
            .map(|r| format!(" retired_at={r}"))
            .unwrap_or_default();
        println!(
            "  kid={} status={} created_at={}{retired}",
            k.kid, status, k.created_at
        );
    }
}

fn rotate(path: String) {
    let mut ks = open_keystore(&path, min_retention_from_env());
    let (new_kid, prev_kid) = ks
        .rotate(now_sec())
        .unwrap_or_else(|e| die(&format!("rotate: {e}")));
    println!(
        "rotated: active_kid={new_kid} previous_kid={prev_kid} (now retiring — stays in JWKS)"
    );
    println!("restart token-service to pick up the new signing key, or use POST /admin/keys/rotate for online rotation");
}

fn retire(path: String, kid: &str) {
    let mut ks = open_keystore(&path, min_retention_from_env());
    ks.retire(kid, now_sec())
        .unwrap_or_else(|e| die(&format!("retire {kid}: {e}")));
    println!("retired: kid={kid} (removed from JWKS; restart token-service)");
    println!(
        "deletable after retention: {}s from now (TOKEN_JWT_MIN_RETENTION_SEC)",
        min_retention_from_env()
    );
}

fn delete(path: String, kid: &str, min_retention: i64) {
    let mut ks = open_keystore(&path, min_retention);
    ks.remove(kid, now_sec())
        .unwrap_or_else(|e| die(&format!("delete {kid}: {e}")));
    println!("deleted: kid={kid} (retention of {min_retention}s satisfied)");
}

fn backup(path: String, out: &str) {
    let ks = open_keystore(&path, min_retention_from_env());
    ks.write_backup(std::path::Path::new(out))
        .unwrap_or_else(|e| die(&format!("backup: {e}")));
    println!(
        "backup written: {out} ({} keys) — contains PRIVATE KEY MATERIAL: store in protected storage, never commit to git",
        ks.data().keys.len()
    );
}

fn restore(backup_path: &str, keystore_path: &str, min_retention: i64) {
    let ks = token_service::keystore::KeyStore::restore_from_backup(
        std::path::Path::new(backup_path),
        std::path::Path::new(keystore_path),
        min_retention,
    )
    .unwrap_or_else(|e| die(&format!("restore: {e}")));
    println!(
        "restored: {} keys, active_kid={} → {keystore_path}",
        ks.data().keys.len(),
        ks.active().kid
    );
    println!("restart token-service to serve with the restored keys");
}
