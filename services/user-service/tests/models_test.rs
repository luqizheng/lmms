use serde_json::json;
use user_service::models::user::{UserDTO, UserProfile, UserRow};

#[test]
fn user_row_to_dto_includes_all_fields() {
    let row = UserRow {
        id: 1,
        username: "alice".into(),
        email: Some("a@x.com".into()),
        phone: None,
        nickname: Some("Ali".into()),
        status: 1,
        user_code: "USR-001".into(),
        profile_json: "{}".into(),
        created_at: "2026-06-10T00:00:00Z".into(),
        updated_at: "2026-06-10T00:00:00Z".into(),
    };
    let dto: UserDTO = row.into_dto(vec!["admin".into()]);
    assert_eq!(dto.id, 1);
    assert_eq!(dto.username, "alice");
    assert_eq!(dto.user_code, "USR-001");
    assert_eq!(dto.roles, vec!["admin"]);
    assert_eq!(dto.profile, UserProfile::default());
}

#[test]
fn user_profile_default_is_empty() {
    let p = UserProfile::default();
    assert_eq!(serde_json::to_value(&p).unwrap(), json!({}));
}

#[test]
fn user_profile_parses_json_string() {
    let p = UserProfile::from_json(r#"{"dept":"eng"}"#).unwrap();
    assert_eq!(p.dept.as_deref(), Some("eng"));
}
