use axum::http::HeaderMap;
use jwt_ctx::Principal;
use std::collections::HashSet;

#[test]
fn parses_all_headers() {
    let mut h = HeaderMap::new();
    h.insert("x-user-id", "user-uuid-1".parse().unwrap());
    h.insert("x-user-code", "aB3xY9zQ".parse().unwrap());
    h.insert("x-user-dept", "7".parse().unwrap());
    h.insert("x-user-roles", "admin,sales".parse().unwrap());
    h.insert("x-user-perms", "user.read,user.write".parse().unwrap());
    let p = Principal::from_headers(&h).unwrap();
    assert_eq!(p.user_id, "user-uuid-1");
    assert_eq!(p.user_code, "aB3xY9zQ");
    assert_eq!(p.dept_id, Some(7));
    assert_eq!(p.roles, vec!["admin".to_string(), "sales".to_string()]);
    assert_eq!(
        p.permissions,
        HashSet::from(["user.read".into(), "user.write".into()])
    );
}

#[test]
fn missing_user_id_is_error() {
    let h = HeaderMap::new();
    assert!(Principal::from_headers(&h).is_err());
}

#[test]
fn optional_fields_default() {
    let mut h = HeaderMap::new();
    h.insert("x-user-id", "u".parse().unwrap());
    let p = Principal::from_headers(&h).unwrap();
    assert_eq!(p.user_code, "");
    assert_eq!(p.dept_id, None);
    assert!(p.roles.is_empty());
    assert!(p.permissions.is_empty());
}
