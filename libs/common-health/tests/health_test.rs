use axum::body::Body;
use axum::http::{Request, StatusCode};
use digua_common_health::{health_routes, HealthState};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn live_returns_ok_without_any_dependencies() {
    let state = HealthState::new("test-service");
    let app = health_routes(state);
    let res = app
        .oneshot(
            Request::builder()
                .uri("/health/live")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"OK");
}

#[tokio::test]
async fn ready_returns_ok_when_no_dependencies_configured() {
    let state = HealthState::new("test-service");
    let app = health_routes(state);
    let res = app
        .oneshot(
            Request::builder()
                .uri("/health/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn ready_returns_503_after_mark_shutting_down() {
    let state = HealthState::new("test-service");
    state.mark_shutting_down();
    let app = health_routes(state);
    let res = app
        .oneshot(
            Request::builder()
                .uri("/health/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn legacy_health_returns_ok() {
    let state = HealthState::new("test-service");
    let app = health_routes(state);
    let res = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
