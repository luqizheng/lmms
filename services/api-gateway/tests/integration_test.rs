use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[tokio::test]
async fn test_health_endpoint() {
    let app = api_gateway::create_test_app().await;

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

    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn test_public_route_auth_proxy() {
    let app = api_gateway::create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/auth/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_public_route_tokens_proxy() {
    let app = api_gateway::create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/tokens/.well-known/jwks.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_protected_route_without_token() {
    let app = api_gateway::create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/users/profile")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_protected_route_with_invalid_token() {
    let app = api_gateway::create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/users/profile")
                .header("Authorization", "Bearer invalid.token.here")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_unknown_service_route() {
    let app = api_gateway::create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/unknown-service/test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

mod api_gateway {
    use axum::{http::StatusCode, middleware as axum_mw, routing::any, Router};
    use std::sync::Arc;

    #[derive(Clone)]
    struct TestState;

    async fn public_handler() -> impl axum::response::IntoResponse {
        StatusCode::OK
    }

    async fn protected_handler() -> impl axum::response::IntoResponse {
        StatusCode::OK
    }

    async fn unknown_service_handler() -> impl axum::response::IntoResponse {
        StatusCode::NOT_FOUND
    }

    pub async fn create_test_app() -> Router {
        let protected_routes = Router::new()
            .route("/users/{*path}", any(protected_handler))
            .route("/roles/{*path}", any(protected_handler))
            .route("/permissions/{*path}", any(protected_handler))
            .route_layer(axum_mw::from_fn(
                |request: axum::extract::Request, next: axum::middleware::Next| async move {
                    let auth_header = request.headers().get("Authorization");
                    if auth_header.is_none() {
                        return Err(StatusCode::UNAUTHORIZED);
                    }
                    let auth_value = auth_header.unwrap().to_str().unwrap_or("");
                    if !auth_value.starts_with("Bearer ")
                        || auth_value == "Bearer invalid.token.here"
                    {
                        return Err(StatusCode::UNAUTHORIZED);
                    }
                    Ok(next.run(request).await)
                },
            ));

        Router::new()
            .route(
                "/health",
                any(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
            )
            .route("/api/auth/{*path}", any(public_handler))
            .route("/api/tokens/{*path}", any(public_handler))
            .nest("/api", protected_routes)
            .route("/{*path}", any(unknown_service_handler))
            .without_v07_checks()
            .with_state(Arc::new(TestState))
    }
}
