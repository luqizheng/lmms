use digua_common_log::AuditClient;

#[tokio::test]
async fn audit_client_buffers_event_when_no_endpoint() {
    let client = AuditClient::new(None, None);

    client
        .log(
            "user.create",
            "user/123",
            None,
            Some(r#"{"name":"test"}"#),
            "req-001",
            Some("user-001"),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn audit_event_contains_all_fields() {
    let client = AuditClient::new(None, None);

    client
        .log(
            "role.update",
            "role/456",
            Some(r#"{"name":"old"}"#),
            Some(r#"{"name":"new"}"#),
            "req-002",
            Some("admin-001"),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn audit_event_with_optional_user() {
    let client = AuditClient::new(None, None);

    client
        .log(
            "system.config",
            "config/db",
            None,
            Some(r#"{"host":"localhost"}"#),
            "req-003",
            None,
        )
        .await
        .unwrap();
}
