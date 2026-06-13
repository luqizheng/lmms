use iam_service::{config::load_config, db, routes, state::AppState};
use axum::{http::{Request, StatusCode}, body::Body};
use tower::ServiceExt;

#[tokio::test]
async fn test_health_endpoint() {
    let config = load_config().unwrap();
    let db_pool = db::init_pool(&config.database).await.unwrap();
    let redis_client = common_cache::new_redis_client(&config.redis).await.unwrap();
    let state = AppState::new(db_pool, redis_client, config);
    
    let app = routes::build_router(state);
    
    let response = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_login_endpoint() {
    let config = load_config().unwrap();
    let db_pool = db::init_pool(&config.database).await.unwrap();
    let redis_client = common_cache::new_redis_client(&config.redis).await.unwrap();
    let state = AppState::new(db_pool, redis_client, config);
    
    let app = routes::build_router(state);
    
    let body = serde_json::json!({
        "username": "test",
        "password": "test"
    });
    
    let response = app
        .oneshot(Request::builder()
            .uri("/iam/auth/login")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap())
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
