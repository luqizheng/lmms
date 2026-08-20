use audit_service::models::QueryOperationLogs;

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
