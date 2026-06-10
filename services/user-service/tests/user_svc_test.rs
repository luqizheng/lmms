use user_service::models::user::UserRow;
use user_service::service::user_svc::build_user_dto;

#[test]
fn build_user_dto_with_empty_roles() {
    let row = UserRow {
        id: 7,
        username: "bob".into(),
        email: None,
        phone: None,
        nickname: None,
        status: 1,
        user_code: "USR-007".into(),
        profile_json: "".into(),
        created_at: "2026-06-10T00:00:00Z".into(),
        updated_at: "2026-06-10T00:00:00Z".into(),
    };
    let dto = build_user_dto(row, vec![]);
    assert_eq!(dto.id, 7);
    assert!(dto.roles.is_empty());
}

#[test]
fn build_user_dto_with_multiple_roles() {
    let row = UserRow {
        id: 7,
        username: "bob".into(),
        email: None,
        phone: None,
        nickname: None,
        status: 1,
        user_code: "USR-007".into(),
        profile_json: r#"{"dept":"eng"}"#.into(),
        created_at: "2026-06-10T00:00:00Z".into(),
        updated_at: "2026-06-10T00:00:00Z".into(),
    };
    let dto = build_user_dto(row, vec!["admin".into(), "ops".into()]);
    assert_eq!(dto.roles.len(), 2);
    assert_eq!(dto.profile.dept.as_deref(), Some("eng"));
}
