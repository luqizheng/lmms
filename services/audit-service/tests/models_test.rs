use audit_service::models::{
    CreateOperationLog, 
    CreateLoginLog, 
    QueryOperationLogs,
    QueryLoginLogs,
};

#[test]
fn create_operation_log_serialization() {
    let log = CreateOperationLog {
        user_id: "u123".to_string(),
        action: "user.update".to_string(),
        resource: "user/456".to_string(),
        resource_id: Some("456".to_string()),
        before_value: Some(r#"{"name":"old"}"#.to_string()),
        after_value: Some(r#"{"name":"new"}"#.to_string()),
        request_id: "req-001".to_string(),
        ip_address: Some("192.168.1.1".to_string()),
        user_agent: Some("Mozilla/5.0".to_string()),
    };

    let json = serde_json::to_string(&log).unwrap();
    assert!(json.contains("u123"));
    assert!(json.contains("user.update"));
}

#[test]
fn query_operation_logs_defaults() {
    let query = QueryOperationLogs {
        user_id: None,
        action: None,
        resource: None,
        start_time: None,
        end_time: None,
        page: None,
        size: None,
    };

    assert!(query.page.is_none());
    assert!(query.size.is_none());
}

#[test]
fn create_login_log_serialization() {
    let log = CreateLoginLog {
        user_id: Some("u123".to_string()),
        username: "admin".to_string(),
        success: true,
        fail_reason: None,
        ip_address: "192.168.1.1".to_string(),
        location: Some("Beijing".to_string()),
        user_agent: Some("Mozilla/5.0".to_string()),
        request_id: "req-002".to_string(),
    };

    let json = serde_json::to_string(&log).unwrap();
    assert!(json.contains("admin"));
    assert!(json.contains("192.168.1.1"));
}

#[test]
fn login_log_fail_reason() {
    let log = CreateLoginLog {
        user_id: None,
        username: "admin".to_string(),
        success: false,
        fail_reason: Some("invalid password".to_string()),
        ip_address: "192.168.1.1".to_string(),
        location: None,
        user_agent: Some("curl/7.68".to_string()),
        request_id: "req-003".to_string(),
    };

    assert!(!log.success);
    assert_eq!(log.fail_reason.unwrap(), "invalid password");
}
