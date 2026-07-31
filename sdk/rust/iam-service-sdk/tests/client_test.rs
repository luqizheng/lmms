use digua_iam_sdk::{
    CreateRoleRequest, CreateUserRequest, IamClient, ListRolesQuery, ListUsersQuery,
    UpdateUserRequest,
};
use digua_http_client::SdkError;
use serde_json::json;
use std::sync::{Arc, Mutex};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn bearer_client(server: &MockServer, token: &str) -> IamClient {
    let token = Arc::new(Mutex::new(Some(token.to_string())));
    IamClient::new(server.uri(), move || token.lock().unwrap().clone())
}

#[tokio::test]
async fn login_posts_credentials_and_returns_tokens() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/iam/auth/login"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "at-1",
            "refresh_token": "rt-1",
            "token_type": "Bearer",
            "expires_in": 3600,
            "user": {
                "id": "u-1",
                "username": "alice",
                "nickname": "Alice",
                "roles": ["admin"],
                "permissions": ["iam:user:read"]
            }
        })))
        .mount(&server)
        .await;

    let (client, login) = IamClient::login(server.uri(), "alice", "secret").await.unwrap();
    assert_eq!(login.access_token, "at-1");
    assert_eq!(login.user.username, "alice");
    assert_eq!(login.user.id, "u-1");
    assert_eq!(client.base_url().trim_end_matches('/'), server.uri().trim_end_matches('/'));
}

#[tokio::test]
async fn refresh_posts_refresh_token_and_returns_new_pair() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/iam/auth/refresh"))
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
    let refresh = client.refresh("rt-1").await.unwrap();
    assert_eq!(refresh.access_token, "at-2");
    assert_eq!(refresh.refresh_token, "rt-2");
}

#[tokio::test]
async fn list_users_builds_query_string_and_parses_paged_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/iam/users"))
        .and(header("authorization", "Bearer at-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "id": "u-1",
                "username": "alice",
                "nickname": "Alice",
                "status": "active"
            }],
            "total": 1,
            "page": 0,
            "size": 20
        })))
        .mount(&server)
        .await;

    let client = bearer_client(&server, "at-1");
    let page = client
        .list_users(&ListUsersQuery::new().keyword("alice").page(0).size(20))
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].username, "alice");
    assert_eq!(page.items[0].status.as_deref(), Some("active"));
}

#[tokio::test]
async fn create_user_posts_body_and_returns_user_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/iam/users"))
        .and(header("authorization", "Bearer at-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "u-new",
            "username": "bob",
            "nickname": "Bob",
            "email": "bob@example.com",
            "status": "active"
        })))
        .mount(&server)
        .await;

    let client = bearer_client(&server, "at-1");
    let req = CreateUserRequest {
        username: "bob".into(),
        password: "secret".into(),
        nickname: "Bob".into(),
        email: Some("bob@example.com".into()),
        phone: None,
        avatar: None,
        tenant_slug: None,
    };
    let user = client.create_user(&req).await.unwrap();
    assert_eq!(user.id, "u-new");
    assert_eq!(user.username, "bob");
}

#[tokio::test]
async fn get_user_returns_user_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/iam/users/u-1"))
        .and(header("authorization", "Bearer at-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "u-1",
            "username": "alice",
            "nickname": "Alice"
        })))
        .mount(&server)
        .await;

    let client = bearer_client(&server, "at-1");
    let user = client.get_user("u-1").await.unwrap();
    assert_eq!(user.username, "alice");
}

#[tokio::test]
async fn change_password_posts_old_and_new() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/iam/auth/password"))
        .and(header("authorization", "Bearer at-1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = bearer_client(&server, "at-1");
    client.change_password("old", "new").await.unwrap();
}

#[tokio::test]
async fn list_roles_parses_paged_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/iam/roles"))
        .and(header("authorization", "Bearer at-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "id": "r-1",
                "name": "Admin",
                "code": "admin",
                "data_scope": 1,
                "status": "active"
            }],
            "total": 1,
            "page": 0,
            "size": 20
        })))
        .mount(&server)
        .await;

    let client = bearer_client(&server, "at-1");
    let page = client
        .list_roles(&ListRolesQuery::new().keyword("admin"))
        .await
        .unwrap();
    assert_eq!(page.items[0].code, "admin");
    assert_eq!(page.items[0].data_scope, 1);
}

#[tokio::test]
async fn create_role_posts_body_and_returns_role() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/iam/roles"))
        .and(header("authorization", "Bearer at-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "r-new",
            "name": "Viewer",
            "code": "viewer",
            "data_scope": 1,
            "status": "active"
        })))
        .mount(&server)
        .await;

    let client = bearer_client(&server, "at-1");
    let req = CreateRoleRequest {
        name: "Viewer".into(),
        code: "viewer".into(),
        data_scope: 1,
        description: Some("Read-only".into()),
    };
    let role = client.create_role(&req).await.unwrap();
    assert_eq!(role.code, "viewer");
}

#[tokio::test]
async fn update_user_posts_patch_body() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/iam/users/u-1"))
        .and(header("authorization", "Bearer at-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "u-1",
            "username": "alice",
            "nickname": "Alice New",
            "status": "active"
        })))
        .mount(&server)
        .await;

    let client = bearer_client(&server, "at-1");
    let req = UpdateUserRequest {
        nickname: Some("Alice New".into()),
        email: None,
        phone: None,
        avatar: None,
        status: None,
    };
    let user = client.update_user("u-1", &req).await.unwrap();
    assert_eq!(user.nickname, "Alice New");
}

#[tokio::test]
async fn list_permissions_parses_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/iam/permissions"))
        .and(header("authorization", "Bearer at-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "id": "p-1",
                "code": "iam:user:read",
                "name": "Read User",
                "category": 1,
                "sort_order": 0,
                "status": "active"
            }],
            "total": 1,
            "page": 0,
            "size": 20
        })))
        .mount(&server)
        .await;

    let client = bearer_client(&server, "at-1");
    let list = client.list_permissions().await.unwrap();
    assert_eq!(list.items[0].code, "iam:user:read");
}

#[tokio::test]
async fn login_401_maps_to_auth_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/iam/auth/login"))
        .respond_with(ResponseTemplate::new(401).set_body_string("invalid credentials"))
        .mount(&server)
        .await;

    let res = IamClient::login(server.uri(), "alice", "wrong").await;
    let err = match res {
        Ok(_) => panic!("expected Auth error, got Ok"),
        Err(e) => e,
    };
    match err {
        SdkError::Auth { status, .. } => assert_eq!(status, 401),
        other => panic!("expected SdkError::Auth, got {:?}", other),
    }
}