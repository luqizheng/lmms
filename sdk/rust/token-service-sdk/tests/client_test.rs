use digua_token_sdk::{
    AccessRequest, CreateApiKeyRequest, CreateServiceJwtGrantRequest, ListApiKeysQuery,
    ListServiceJwtGrantsQuery, RefreshRequest, RevokeRequest, SetServiceJwtGrantStatusRequest,
    TokenClient,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn bearer_client(server: &MockServer, token: &str) -> TokenClient {
    let token = Arc::new(Mutex::new(Some(token.to_string())));
    TokenClient::new(server.uri(), move || token.lock().unwrap().clone())
}

#[tokio::test]
async fn access_posts_grant_and_returns_tokens() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tokens/access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "at-1",
            "refresh_token": "rt-1",
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "ignored");
    let req = AccessRequest {
        grant_type: "authorization_code".into(),
        code: "abc".into(),
        client_id: "c1".into(),
        client_secret: "s1".into(),
    };
    let resp = client.access(&req).await.unwrap();
    assert_eq!(resp.access_token, "at-1");
}

#[tokio::test]
async fn refresh_posts_refresh_token_and_returns_new_pair() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tokens/refresh"))
        .and(header("authorization", "Bearer at-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "at-2",
            "refresh_token": "rt-2",
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "at-1");
    let req = RefreshRequest {
        refresh_token: "rt-1".into(),
        client_id: "c1".into(),
        client_secret: "s1".into(),
    };
    let resp = client.refresh(&req).await.unwrap();
    assert_eq!(resp.refresh_token, "rt-2");
}

#[tokio::test]
async fn revoke_posts_token_jti_and_returns_success() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tokens/revoke"))
        .and(header("authorization", "Bearer at-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "success": true })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "at-1");
    let req = RevokeRequest {
        token_jti: "jti-1".into(),
        expires_in: 1234567890,
    };
    let resp = client.revoke(&req).await.unwrap();
    assert!(resp.success);
}

#[tokio::test]
async fn list_api_keys_builds_query_and_parses_paged_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api-keys"))
        .and(header("authorization", "Bearer at-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "api_key_id": "ak-1",
                "name": "service-a",
                "status": "active",
                "permissions": ["iam:user:read"],
                "grant_types": ["client_credentials"]
            }],
            "total": 1,
            "page": 0,
            "size": 20
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "at-1");
    let resp = client
        .list_api_keys(&ListApiKeysQuery::new().keyword("service").page(0).size(20))
        .await
        .unwrap();
    assert_eq!(resp.total, 1);
    assert_eq!(resp.items[0].api_key_id.as_deref(), Some("ak-1"));
}

#[tokio::test]
async fn create_api_key_returns_id_and_secret() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api-keys"))
        .and(header("authorization", "Bearer at-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "api_key_id": "ak-new",
            "api_key_secret": "secret-once"
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "at-1");
    let req = CreateApiKeyRequest {
        name: "service-b".into(),
        permissions: vec!["iam:user:read".into()],
        grant_types: vec!["client_credentials".into()],
    };
    let created = client.create_api_key(&req).await.unwrap();
    assert_eq!(created.api_key_id, "ak-new");
    assert_eq!(created.api_key_secret, "secret-once");
}

#[tokio::test]
async fn access_401_maps_to_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tokens/access"))
        .respond_with(ResponseTemplate::new(401).set_body_string("invalid client"))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "ignored");
    let req = AccessRequest {
        grant_type: "client_credentials".into(),
        code: "x".into(),
        client_id: "c".into(),
        client_secret: "bad".into(),
    };
    let err = client.access(&req).await.expect_err("expected Auth error");
    match err {
        digua_http_client::SdkError::Auth { status, .. } => assert_eq!(status, 401),
        other => panic!("expected Auth, got {:?}", other),
    }
}

// ── Service JWT audience grants ─────────────────────────────────────────────

#[tokio::test]
async fn list_service_jwt_grants_sends_filters_and_decodes_page() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/admin/service-jwt-grants"))
        .and(wiremock::matchers::query_param("owner", "svc-a"))
        .and(wiremock::matchers::query_param("status", "active"))
        .and(header("authorization", "Bearer svc-jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "owner": "svc-a",
                "audience": "iam-service",
                "status": "active",
                "system_managed": false,
                "created_at": "2026-11-26T00:00:00Z",
                "updated_at": "2026-11-26T00:00:00Z"
            }],
            "total": 1,
            "page": 1,
            "size": 20
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "svc-jwt");
    let q = ListServiceJwtGrantsQuery::new()
        .owner("svc-a")
        .status("active");
    let resp = client.list_service_jwt_grants(&q).await.unwrap();
    assert_eq!(resp.total, 1);
    let g = &resp.items[0];
    assert_eq!(g.owner, "svc-a");
    assert_eq!(g.audience, "iam-service");
    assert_eq!(g.status, "active");
    assert!(!g.system_managed);
}

#[tokio::test]
async fn create_service_jwt_grant_posts_body_and_returns_row() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/admin/service-jwt-grants"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "owner": "svc-a",
            "audience": "iam-service",
            "status": "active",
            "system_managed": false
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "svc-jwt");
    let req = CreateServiceJwtGrantRequest {
        owner: "svc-a".into(),
        audience: "iam-service".into(),
    };
    let g = client.create_service_jwt_grant(&req).await.unwrap();
    assert_eq!(g.status, "active");
}

#[tokio::test]
async fn create_service_jwt_grant_maps_409_to_validation_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/admin/service-jwt-grants"))
        .respond_with(ResponseTemplate::new(409).set_body_string("grant already exists"))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "svc-jwt");
    let req = CreateServiceJwtGrantRequest {
        owner: "svc-a".into(),
        audience: "iam-service".into(),
    };
    let err = client.create_service_jwt_grant(&req).await.unwrap_err();
    assert!(
        matches!(
            err,
            digua_http_client::SdkError::Validation { status: 409, .. }
        ),
        "409 must surface as Validation: {err}"
    );
}

#[tokio::test]
async fn set_service_jwt_grant_status_puts_body() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/admin/service-jwt-grants/svc-a/iam-service/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "owner": "svc-a",
            "audience": "iam-service",
            "status": "disabled",
            "system_managed": false
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "svc-jwt");
    let req = SetServiceJwtGrantStatusRequest {
        status: "disabled".into(),
    };
    let g = client
        .set_service_jwt_grant_status("svc-a", "iam-service", &req)
        .await
        .unwrap();
    assert_eq!(g.status, "disabled");
}

#[tokio::test]
async fn get_and_delete_service_jwt_grant_hit_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/admin/service-jwt-grants/svc-a/iam-service"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "owner": "svc-a",
            "audience": "iam-service",
            "status": "active",
            "system_managed": true
        })))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/admin/service-jwt-grants/svc-a/iam-service"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "svc-jwt");
    let g = client
        .get_service_jwt_grant("svc-a", "iam-service")
        .await
        .unwrap();
    assert!(g.system_managed, "system_managed must round-trip");
    client
        .delete_service_jwt_grant("svc-a", "iam-service")
        .await
        .unwrap();
}

// ── R1-03：URL 编码——断言最终发出的 HTTP 请求路径与 Query ───────────────────

/// 组合攻击字符（含 `/ ? # & %` 结构字符 + Unicode）：编码后必须整体仍是
/// 单个 path segment / 单个 query value，不能改变请求结构。
const HOSTILE: &str = "a/b?c#d&e%f服务";
/// `HOSTILE` 的期望 percent-encoding（UTF-8 逐字节，`服` = E6 9C 8D、`务` = E5 8A A1）。
const HOSTILE_ENCODED: &str = "a%2Fb%3Fc%23d%26e%25f%E6%9C%8D%E5%8A%A1";

/// 宽挂载 catch-all（method 限定），请求后用 `received_requests` 对
/// **原始（未解码）URL** 做显式断言——直接检查最终发出的请求结构，
/// 不经由 matcher 间接验证。`url::Url::path()/query()` 保留 percent-encoding。
async fn mount_catch_all(server: &MockServer, method_name: &str, status: u16) {
    Mock::given(method(method_name))
        .respond_with(ResponseTemplate::new(status).set_body_json(json!({
            "owner": "o", "audience": "a", "status": "active",
            "system_managed": false, "items": [],
            "total": 0, "page": 1, "size": 20
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn hostile_path_params_cannot_change_request_structure() {
    let server = MockServer::start().await;
    mount_catch_all(&server, "GET", 200).await;
    mount_catch_all(&server, "PUT", 200).await;
    mount_catch_all(&server, "DELETE", 204).await;

    let client = bearer_client(&server, "svc-jwt");
    let g = client
        .get_service_jwt_grant(HOSTILE, HOSTILE)
        .await
        .unwrap();
    assert_eq!(g.status, "active");
    let g = client
        .set_service_jwt_grant_status(
            HOSTILE,
            HOSTILE,
            &SetServiceJwtGrantStatusRequest {
                status: "disabled".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(g.status, "active");
    client
        .delete_service_jwt_grant(HOSTILE, HOSTILE)
        .await
        .unwrap();

    let reqs = server.received_requests().await.expect("received requests");
    assert_eq!(reqs.len(), 3);
    let expected_get_delete =
        format!("/admin/service-jwt-grants/{HOSTILE_ENCODED}/{HOSTILE_ENCODED}");
    let expected_put = format!("{expected_get_delete}/status");
    // `/ ? # & %` 与 Unicode 全部转义：owner/audience 仍是单段路径，无结构注入
    assert_eq!(reqs[0].url.path(), expected_get_delete, "GET path");
    assert_eq!(reqs[1].url.path(), expected_put, "PUT path");
    assert_eq!(reqs[2].url.path(), expected_get_delete, "DELETE path");
    // 无 query 泄漏（编码后的 `?`/#` 不得被解释为结构分隔）
    for r in &reqs {
        assert!(
            r.url.query().is_none(),
            "path params must not leak into query: {}",
            r.url
        );
    }
}

#[tokio::test]
async fn hostile_query_params_cannot_change_request_structure() {
    let server = MockServer::start().await;
    mount_catch_all(&server, "GET", 200).await;

    let client = bearer_client(&server, "svc-jwt");
    let q = ListServiceJwtGrantsQuery::new()
        .owner(HOSTILE)
        .audience(HOSTILE)
        .status("active")
        .page(2)
        .size(5);
    client.list_service_jwt_grants(&q).await.unwrap();

    let reqs = server.received_requests().await.expect("received requests");
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].url.path(), "/admin/service-jwt-grants");
    // query 值全部 percent-encode：`&`/`=`/`%` 不得改变参数切分
    assert_eq!(
        reqs[0].url.query(),
        Some(
            format!(
                "owner={HOSTILE_ENCODED}&audience={HOSTILE_ENCODED}&status=active&page=2&size=5&"
            )
            .as_str()
        ),
        "raw query must be encoded (to_query_string keeps a trailing &)"
    );
}

#[tokio::test]
async fn plain_params_keep_semantics_after_encoding() {
    let server = MockServer::start().await;
    mount_catch_all(&server, "GET", 200).await;

    let client = bearer_client(&server, "svc-jwt");
    let g = client
        .get_service_jwt_grant("svc-a", "aud_1")
        .await
        .unwrap();
    assert_eq!(g.status, "active");
    let q = ListServiceJwtGrantsQuery::new()
        .owner("svc-a")
        .audience("aud_1")
        .status("active")
        .page(1)
        .size(20);
    client.list_service_jwt_grants(&q).await.unwrap();

    let reqs = server.received_requests().await.expect("received requests");
    assert_eq!(reqs.len(), 2);
    // 正常合法输入：unreserved 字符不转义，语义不变
    assert_eq!(
        reqs[0].url.path(),
        "/admin/service-jwt-grants/svc-a/aud_1",
        "GET path"
    );
    assert_eq!(
        reqs[1].url.query(),
        Some("owner=svc-a&audience=aud_1&status=active&page=1&size=20&"),
        "plain query stays verbatim"
    );
}
