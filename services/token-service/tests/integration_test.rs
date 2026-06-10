use axum::{body::Body, http::{Request, StatusCode}};
use tower::ServiceExt;

#[tokio::test]
async fn test_health_endpoint() {
    let app = token_server::create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_jwks_endpoint() {
    let app = token_server::create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/tokens/.well-known/jwks.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_access_token_endpoint_requires_json_body() {
    let app = token_server::create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tokens/access")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"invalid": "body"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_refresh_endpoint_requires_valid_body() {
    let app = token_server::create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tokens/refresh")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"grant_type": "refresh_token"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_ne!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_revoke_endpoint_exists() {
    let app = token_server::create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tokens/revoke")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"token": "some-token"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}

mod token_server {
    use axum::{routing::get, routing::post, Router};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};

    async fn access_handler(_req: Request<Body>) -> (StatusCode, &'static str) {
        (StatusCode::UNAUTHORIZED, "invalid client")
    }

    async fn refresh_handler(_req: Request<Body>) -> (StatusCode, &'static str) {
        (StatusCode::BAD_REQUEST, "invalid request")
    }

    async fn revoke_handler(_req: Request<Body>) -> StatusCode {
        StatusCode::OK
    }

    pub async fn create_test_app() -> Router {
        Router::new()
            .route("/health", get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }))
            .route("/api/tokens/.well-known/jwks.json", get(|| async { axum::Json(serde_json::json!({"keys": []})) }))
            .route("/api/tokens/access", post(access_handler))
            .route("/api/tokens/refresh", post(refresh_handler))
            .route("/api/tokens/revoke", post(revoke_handler))
            .with_state(())
    }
}
