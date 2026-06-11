use axum::http::{HeaderMap, HeaderValue};
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

#[test]
fn has_role_matches_existing() {
    let mut h = HeaderMap::new();
    h.insert("x-user-id", "u".parse().unwrap());
    h.insert("x-user-roles", "admin,ops".parse().unwrap());
    let p = Principal::from_headers(&h).unwrap();
    assert!(p.has_role("admin"));
    assert!(p.has_role("ops"));
    assert!(!p.has_role("guest"));
}

#[test]
fn has_role_empty_string_is_filtered() {
    let mut h = HeaderMap::new();
    h.insert("x-user-id", "u".parse().unwrap());
    h.insert("x-user-roles", "admin,,ops,".parse().unwrap());
    let p = Principal::from_headers(&h).unwrap();
    assert_eq!(p.roles, vec!["admin".to_string(), "ops".to_string()]);
    assert!(!p.has_role(""));
}

#[test]
fn has_permission_matches_existing() {
    let mut h = HeaderMap::new();
    h.insert("x-user-id", "u".parse().unwrap());
    h.insert("x-user-perms", "user.read,user.write".parse().unwrap());
    let p = Principal::from_headers(&h).unwrap();
    assert!(p.has_permission("user.read"));
    assert!(p.has_permission("user.write"));
    assert!(!p.has_permission("user.delete"));
}

#[test]
fn has_permission_empty_string_is_filtered() {
    let mut h = HeaderMap::new();
    h.insert("x-user-id", "u".parse().unwrap());
    h.insert("x-user-perms", ",user.read,".parse().unwrap());
    let p = Principal::from_headers(&h).unwrap();
    assert_eq!(p.permissions.len(), 1);
    assert!(p.has_permission("user.read"));
    assert!(!p.has_permission(""));
}

#[test]
fn invalid_dept_id_becomes_none() {
    let mut h = HeaderMap::new();
    h.insert("x-user-id", "u".parse().unwrap());
    h.insert("x-user-dept", "not-a-number".parse().unwrap());
    let p = Principal::from_headers(&h).unwrap();
    assert_eq!(p.dept_id, None);
}

#[test]
fn invalid_user_id_header_value_is_error() {
    let mut h = HeaderMap::new();
    h.insert("x-user-id", HeaderValue::from_bytes(&[0xff, 0xfe, 0xfd]).unwrap());
    assert!(Principal::from_headers(&h).is_err());
}
