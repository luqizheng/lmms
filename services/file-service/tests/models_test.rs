use file_service::models::{CreatePresignUrl, PresignOperation, QueryFiles, UploadFileMeta};

#[test]
fn upload_file_meta_serialization() {
    let meta = UploadFileMeta {
        filename: "test.txt".to_string(),
        content_type: Some("text/plain".to_string()),
        tenant_id: Some(123),
        user_id: Some("user-456".to_string()),
        is_temporary: Some(false),
        expires_in_seconds: None,
    };

    let json = serde_json::to_string(&meta).unwrap();
    assert!(json.contains("test.txt"));
    assert!(json.contains("text/plain"));
}

#[test]
fn upload_file_meta_temporary() {
    let meta = UploadFileMeta {
        filename: "temp.txt".to_string(),
        content_type: None,
        tenant_id: None,
        user_id: None,
        is_temporary: Some(true),
        expires_in_seconds: Some(3600),
    };

    assert!(meta.is_temporary.unwrap());
    assert_eq!(meta.expires_in_seconds, Some(3600));
}

#[test]
fn query_files_defaults() {
    let query = QueryFiles {
        tenant_id: None,
        user_id: None,
        filename: None,
        page: None,
        size: None,
    };

    assert!(query.page.is_none());
    assert!(query.size.is_none());
}

#[test]
fn query_files_with_filters() {
    let query = QueryFiles {
        tenant_id: Some("123".to_string()),
        user_id: Some("user-456".to_string()),
        filename: Some("report".to_string()),
        page: Some(1),
        size: Some(10),
    };

    assert_eq!(query.tenant_id, Some("123".to_string()));
    assert_eq!(query.size, Some(10));
}

#[test]
fn create_presign_url_get() {
    let req = CreatePresignUrl {
        file_id: "file-123".to_string(),
        operation: PresignOperation::Get,
        expires_in_seconds: Some(3600),
    };

    assert_eq!(req.operation, PresignOperation::Get);
    assert_eq!(req.expires_in_seconds, Some(3600));
}

#[test]
fn create_presign_url_put() {
    let req = CreatePresignUrl {
        file_id: "file-456".to_string(),
        operation: PresignOperation::Put,
        expires_in_seconds: None,
    };

    assert_eq!(req.operation, PresignOperation::Put);
    assert!(req.expires_in_seconds.is_none());
}

#[test]
fn presign_operation_is_strict_and_serializes_lowercase() {
    assert_eq!(
        serde_json::to_string(&PresignOperation::Get).unwrap(),
        "\"get\""
    );
    assert_eq!(
        serde_json::to_string(&PresignOperation::Put).unwrap(),
        "\"put\""
    );
    assert!(serde_json::from_str::<PresignOperation>("\"download\"").is_err());
    assert!(serde_json::from_str::<PresignOperation>("\"delete\"").is_err());
}
