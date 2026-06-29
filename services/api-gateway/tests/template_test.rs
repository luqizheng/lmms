use api_gateway::template::{ExtractError, RenderError, Template};
use std::collections::HashMap;

#[test]
fn extract_one_var_tail() {
    let t = Template::parse("/v1/iam/{rest}").unwrap();
    let d = t.extract("/v1/iam/users/42").unwrap();
    assert_eq!(d["rest"], "users/42");
}

#[test]
fn extract_multi_vars() {
    let t = Template::parse("/a/{x}/b/{y}").unwrap();
    let d = t.extract("/a/foo/b/bar").unwrap();
    assert_eq!(d["x"], "foo");
    assert_eq!(d["y"], "bar");
}

#[test]
fn extract_var_value_can_contain_slash() {
    let t = Template::parse("/v1/iam/{rest}").unwrap();
    let d = t.extract("/v1/iam/a/b/c").unwrap();
    assert_eq!(d["rest"], "a/b/c");
}

#[test]
fn extract_literal_mismatch() {
    let t = Template::parse("/v1/iam/{rest}").unwrap();
    let err = t.extract("/v2/iam/x").unwrap_err();
    assert!(matches!(err, ExtractError::LiteralMismatch(_, _)));
}

#[test]
fn extract_anchor_not_found() {
    let t = Template::parse("/a/{x}/b/{y}").unwrap();
    let err = t.extract("/a/foo/no-b-here").unwrap_err();
    assert!(matches!(err, ExtractError::AnchorNotFound(_)));
}

#[test]
fn render_path_with_vars() {
    let t = Template::parse("/iam/{rest}").unwrap();
    let mut d = HashMap::new();
    d.insert("rest".to_string(), "users/42".to_string());
    assert_eq!(t.render(&d).unwrap(), "/iam/users/42");
}

#[test]
fn render_with_query() {
    let t = Template::parse("/billing/subs/{sub}/updates?unitId={unit}").unwrap();
    let mut d = HashMap::new();
    d.insert("sub".to_string(), "s1".to_string());
    d.insert("unit".to_string(), "u9".to_string());
    assert_eq!(t.render(&d).unwrap(), "/billing/subs/s1/updates?unitId=u9");
}

#[test]
fn render_missing_var() {
    let t = Template::parse("/a/{x}").unwrap();
    let d = HashMap::new();
    assert!(matches!(
        t.render(&d).unwrap_err(),
        RenderError::MissingVar(_)
    ));
}
