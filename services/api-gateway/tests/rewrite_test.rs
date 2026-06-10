use api_gateway::config::RewriteSpec;
use api_gateway::rewrite::apply;

#[test]
fn strip_prefix_removes_prefix() {
    let rules = vec![RewriteSpec::StripPrefix {
        prefix: "/api".into(),
    }];
    let mut req = axum::http::Request::builder()
        .uri("/api/users/1")
        .body(axum::body::Body::empty())
        .unwrap();
    apply(&rules, &mut req).unwrap();
    assert_eq!(req.uri().path(), "/users/1");
}

#[test]
fn strip_prefix_when_empty_returns_root() {
    let rules = vec![RewriteSpec::StripPrefix {
        prefix: "/api".into(),
    }];
    let mut req = axum::http::Request::builder()
        .uri("/api")
        .body(axum::body::Body::empty())
        .unwrap();
    apply(&rules, &mut req).unwrap();
    assert_eq!(req.uri().path(), "/");
}

#[test]
fn header_set_overwrites() {
    let rules = vec![RewriteSpec::HeaderSet {
        name: "x-test".into(),
        value: "v".into(),
    }];
    let mut req = axum::http::Request::builder()
        .uri("/x")
        .header("x-test", "old")
        .body(axum::body::Body::empty())
        .unwrap();
    apply(&rules, &mut req).unwrap();
    assert_eq!(req.headers().get("x-test").unwrap(), "v");
}

#[test]
fn header_add_appends() {
    let rules = vec![RewriteSpec::HeaderAdd {
        name: "x-test".into(),
        value: "v2".into(),
    }];
    let mut req = axum::http::Request::builder()
        .uri("/x")
        .header("x-test", "v1")
        .body(axum::body::Body::empty())
        .unwrap();
    apply(&rules, &mut req).unwrap();
    let vals: Vec<&str> = req
        .headers()
        .get_all("x-test")
        .iter()
        .filter_map(|h| h.to_str().ok())
        .collect();
    assert_eq!(vals, vec!["v1", "v2"]);
}

#[test]
fn redirect_short_circuits() {
    let rules = vec![
        RewriteSpec::HeaderSet {
            name: "x-a".into(),
            value: "1".into(),
        },
        RewriteSpec::Redirect {
            status: 301,
            to: "/new".into(),
        },
    ];
    let mut req = axum::http::Request::builder()
        .uri("/old")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = apply(&rules, &mut req).unwrap_err();
    assert_eq!(resp.status(), axum::http::StatusCode::MOVED_PERMANENTLY);
    assert_eq!(resp.headers().get("location").unwrap(), "/new");
}

#[test]
fn strip_prefix_missing_returns_error() {
    let rules = vec![RewriteSpec::StripPrefix {
        prefix: "/api".into(),
    }];
    let mut req = axum::http::Request::builder()
        .uri("/users/1")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = apply(&rules, &mut req).unwrap_err();
    assert_eq!(resp.status(), axum::http::StatusCode::BAD_REQUEST);
}
