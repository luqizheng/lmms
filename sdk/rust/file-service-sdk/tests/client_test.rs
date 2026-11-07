use digua_file_sdk::{CreatePresignUrl, FileClient, ListFilesQuery};
use serde_json::json;
use std::sync::{Arc, Mutex};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn bearer_client(server: &MockServer, token: &str) -> FileClient {
    let token = Arc::new(Mutex::new(Some(token.to_string())));
    FileClient::new(server.uri(), move || token.lock().unwrap().clone())
}

#[tokio::test]
async fn create_presign_posts_body_and_returns_presigned_url() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/file/presign"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "url": "https://storage.example.com/upload/f-1?sig=xxx",
            "expires_at": "2026-07-17T22:00:00Z",
            "operation": "upload"
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "test.jwt");
    let req = CreatePresignUrl {
        file_id: "f-1".into(),
        operation: "upload".into(),
        expires_in_seconds: Some(3600),
    };
    let resp = client.create_presign(&req).await.unwrap();
    assert_eq!(resp.operation, "upload");
    assert!(resp.url.starts_with("https://storage.example.com/upload/"));
}

#[tokio::test]
async fn list_files_builds_query_and_parses_paged_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/file/list"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "id": "f-1",
                "filename": "report.pdf",
                "size": 1024,
                "storage_path": "/uploads/2026/07/report.pdf",
                "tenant_id": "t-1",
                "is_temporary": false,
                "created_at": "2026-07-10T00:00:00Z",
                "updated_at": "2026-07-10T00:00:00Z",
                "content_type": "application/pdf"
            }],
            "total": 1,
            "page": 0,
            "size": 20
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "test.jwt");
    let page = client
        .list_files(&ListFilesQuery::new().filename("report").page(0).size(20))
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].filename, "report.pdf");
    assert_eq!(
        page.items[0].content_type.as_deref(),
        Some("application/pdf")
    );
}

#[tokio::test]
async fn get_file_info_returns_file_info() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/file/f-1/info"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "f-1",
            "filename": "doc.txt",
            "size": 42,
            "storage_path": "/uploads/doc.txt",
            "tenant_id": "t-1",
            "is_temporary": false,
            "created_at": "2026-07-10T00:00:00Z",
            "updated_at": "2026-07-10T00:00:00Z"
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "test.jwt");
    let info = client.get_file_info("f-1").await.unwrap();
    assert_eq!(info.id, "f-1");
    assert_eq!(info.size, 42);
}

#[tokio::test]
async fn presign_401_maps_to_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/file/presign"))
        .respond_with(ResponseTemplate::new(401).set_body_string("nope"))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "expired");
    let req = CreatePresignUrl {
        file_id: "f-1".into(),
        operation: "upload".into(),
        expires_in_seconds: None,
    };
    let err = client
        .create_presign(&req)
        .await
        .expect_err("expected Auth");
    match err {
        digua_http_client::SdkError::Auth { status, .. } => assert_eq!(status, 401),
        other => panic!("expected Auth, got {:?}", other),
    }
}

#[tokio::test]
async fn upload_file_sends_multipart_and_parses_file_info() {
    use digua_file_sdk::UploadFileMeta;
    use wiremock::matchers::body_partial_json;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/file/upload"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "f-new",
            "filename": "contract.pdf",
            "size": 5,
            "storage_path": "f-new",
            "tenant_id": "1",
            "is_temporary": false,
            "created_at": "2026-11-07T00:00:00Z",
            "updated_at": "2026-11-07T00:00:00Z"
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "test.jwt");
    let meta = UploadFileMeta {
        filename: "contract.pdf".into(),
        content_type: Some("application/pdf".into()),
        user_id: Some("user-9".into()),
        is_temporary: None,
        expires_in_seconds: None,
    };
    let info = client
        .upload_file(&meta, "contract.pdf", b"hello".to_vec())
        .await
        .unwrap();
    assert_eq!(info.id, "f-new");
    assert_eq!(info.size, 5);

    // The multipart body must carry both fields — verify via the recorded
    // request count + content type of the request the mock saw.
    let received = &server.received_requests().await.unwrap();
    assert_eq!(received.len(), 1);
    let content_type = received[0]
        .headers
        .iter()
        .find(|(name, _)| name.as_str() == "content-type")
        .map(|(_, value)| value.to_str().unwrap_or(""))
        .unwrap_or("");
    assert!(content_type.starts_with("multipart/form-data; boundary="));
    let body = String::from_utf8_lossy(&received[0].body);
    assert!(body.contains("user_id"));
    assert!(body.contains("user-9"));
    assert!(body.contains(&"hello".to_string()) || received[0].body.ends_with(b"hello"));
    let _ = body_partial_json(json!({})); // keep import referenced
}

#[tokio::test]
async fn download_file_returns_bytes_and_parses_disposition() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/file/f-1"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(b"PDFBYTES".to_vec())
                .append_header("content-type", "application/pdf")
                .append_header("content-disposition", "attachment; filename=\"report.pdf\""),
        )
        .mount(&server)
        .await;
    let client = bearer_client(&server, "test.jwt");
    let file = client.download_file("f-1").await.unwrap();
    assert_eq!(file.bytes, b"PDFBYTES".to_vec());
    assert_eq!(file.content_type.as_deref(), Some("application/pdf"));
    assert_eq!(file.filename.as_deref(), Some("report.pdf"));
}

#[tokio::test]
async fn download_file_401_maps_to_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/file/f-1"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "stale.jwt");
    let err = client.download_file("f-1").await.unwrap_err();
    assert!(matches!(err, digua_file_sdk::SdkError::Auth { status: 401, .. }));
}

#[tokio::test]
async fn upload_file_without_bearer_is_auth_error() {
    let server = MockServer::start().await;
    let token = Arc::new(Mutex::new(None::<String>));
    let client = FileClient::new(server.uri(), move || token.lock().unwrap().clone());
    let meta = digua_file_sdk::UploadFileMeta {
        filename: "x.txt".into(),
        ..Default::default()
    };
    let err = client.upload_file(&meta, "x.txt", vec![1]).await.unwrap_err();
    assert!(matches!(err, digua_file_sdk::SdkError::Auth { status: 401, .. }));
    // The mock server must never have been contacted.
    assert!(server.received_requests().await.unwrap().is_empty());
}
