use api_gateway::routes_loader::{load_routes, LoadError};
use std::fs;
use tempfile::tempdir;

fn write(dir: &std::path::Path, name: &str, body: &str) {
    fs::write(dir.join(name), body).unwrap();
}

#[test]
fn loads_basic_route() {
    let d = tempdir().unwrap();
    write(
        d.path(),
        "iam.router.toml",
        r#"
[[route]]
upstream   = "/v1/iam/auth/login"
downstream = "/iam/auth/login"
service    = "iam-service"
public     = true
"#,
    );
    let table = load_routes(d.path()).unwrap();
    assert_eq!(table.routes.len(), 1);
    assert_eq!(table.routes[0].upstream, "/v1/iam/auth/login");
    assert_eq!(table.routes[0].downstream, "/iam/auth/login");
}

#[test]
fn rejects_var_mismatch() {
    let d = tempdir().unwrap();
    write(
        d.path(),
        "x.router.toml",
        r#"
[[route]]
upstream   = "/v1/iam/{rest}"
downstream = "/iam/{other}"
service    = "iam-service"
"#,
    );
    let err = load_routes(d.path()).unwrap_err();
    assert!(matches!(err, LoadError::VarMismatch { .. }));
}

#[test]
fn rejects_duplicate_upstream() {
    let d = tempdir().unwrap();
    write(
        d.path(),
        "x.router.toml",
        r#"
[[route]]
upstream   = "/v1/a"
downstream = "/a"
service    = "s"

[[route]]
upstream   = "/v1/a"
downstream = "/a"
service    = "s"
"#,
    );
    let err = load_routes(d.path()).unwrap_err();
    assert!(matches!(err, LoadError::DuplicateUpstream(_)));
}

#[test]
fn rejects_legacy_match_field() {
    let d = tempdir().unwrap();
    write(
        d.path(),
        "x.router.toml",
        r#"
[[route]]
match   = "/v1/legacy"
service = "s"
"#,
    );
    let err = load_routes(d.path()).unwrap_err();
    assert!(matches!(err, LoadError::Parse { .. }));
}

#[test]
fn returns_empty_when_dir_empty() {
    let dir = tempdir().unwrap();
    let table = load_routes(dir.path()).unwrap();
    assert!(table.routes.is_empty());
}

#[test]
fn sorts_by_literal_length_desc() {
    let d = tempdir().unwrap();
    write(
        d.path(),
        "x.router.toml",
        r#"
[[route]]
upstream   = "/v1/iam/{rest}"
downstream = "/iam/{rest}"
service    = "iam-service"

[[route]]
upstream   = "/v1/iam/auth/login"
downstream = "/iam/auth/login"
service    = "iam-service"
"#,
    );
    let table = load_routes(d.path()).unwrap();
    // 更具体的 /v1/iam/auth/login 应排在前面
    assert_eq!(table.routes[0].upstream, "/v1/iam/auth/login");
    assert_eq!(table.routes[1].upstream, "/v1/iam/{rest}");
}
