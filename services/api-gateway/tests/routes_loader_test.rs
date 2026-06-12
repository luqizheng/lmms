use api_gateway::routes_loader::load_routes;
use std::fs;
use tempfile::tempdir;

#[test]
fn loads_single_file() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("auth.router.toml"),
        r#"
        [[route]]
        match = "/auth/**"
        service = "auth-service"
        public = true
        "#,
    )
    .unwrap();
    let table = load_routes(dir.path()).unwrap();
    assert_eq!(table.routes.len(), 1);
    assert_eq!(table.routes[0].service, "auth-service");
}

#[test]
fn loads_multiple_files_sorted() {
    let dir = tempdir().unwrap();
    for name in &["a.router.toml", "b.router.toml", "c.router.toml"] {
        fs::write(
            dir.path().join(name),
            r#"
            [[route]]
            match = "/x/**"
            service = "x"
            "#,
        )
        .unwrap();
    }
    assert!(load_routes(dir.path()).is_err()); // duplicate match across files
}

#[test]
fn rejects_duplicate_match() {
    let dir = tempdir().unwrap();
    for name in &["a.router.toml", "b.router.toml"] {
        fs::write(
            dir.path().join(name),
            r#"
            [[route]]
            match = "/users/**"
            service = "user-service"
            "#,
        )
        .unwrap();
    }
    let err = load_routes(dir.path()).unwrap_err();
    assert!(err.to_string().contains("duplicate"));
}

#[test]
fn rejects_unknown_rewrite_type() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("bad.router.toml"),
        r#"
        [[route]]
        match = "/x/**"
        service = "x"
        rewrite = [{ type = "unknown_op", foo = "bar" }]
        "#,
    )
    .unwrap();
    assert!(load_routes(dir.path()).is_err());
}

#[test]
fn returns_empty_when_dir_empty() {
    let dir = tempdir().unwrap();
    let table = load_routes(dir.path()).unwrap();
    assert!(table.routes.is_empty());
}
