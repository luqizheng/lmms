use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::middleware as axum_mw;
use axum::routing::get;
use axum::Router;
use authz::require_service_token;
use jwt_simple::algorithms::RS256KeyPair;
use jwt_simple::prelude::*;
use tower::ServiceExt;

async fn echo() -> &'static str {
    "ok"
}

fn make_key_and_token(aud: &str, sub: &str) -> (String, String) {
    let kp = RS256KeyPair::generate(2048).unwrap();
    let pk_pem = kp.public_key().to_pem().unwrap();
    let mut claims: JWTClaims<std::collections::HashMap<String, serde_json::Value>> = JWTClaims::new();
    claims.subject = Some(sub.to_string());
    claims.issuer = Some("token-service".to_string());
    claims.audiences = Some(Audiences::AsString(aud.to_string()));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    claims.expires_at = Some(Duration::from_secs(now + 60));
    claims.custom.insert("token_type".to_string(), serde_json::Value::String("service".to_string()));
    let token = kp.sign(claims).unwrap();
    (pk_pem, token)
}

#[tokio::test]
async fn valid_service_token_passes() {
    let (pk_pem, token) = make_key_and_token("user-service", "credential-login-service");
    let app = Router::new()
        .route("/", get(echo))
        .layer(axum_mw::from_fn(require_service_token(
            "user-service",
            pk_pem,
        )));
    let req = Request::builder()
        .uri("/")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn wrong_audience_is_403() {
    let (pk_pem, token) = make_key_and_token("other-service", "x");
    let app = Router::new()
        .route("/", get(echo))
        .layer(axum_mw::from_fn(require_service_token(
            "user-service",
            pk_pem,
        )));
    let req = Request::builder()
        .uri("/")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn missing_token_is_401() {
    let (pk_pem, _) = make_key_and_token("user-service", "x");
    let app = Router::new()
        .route("/", get(echo))
        .layer(axum_mw::from_fn(require_service_token(
            "user-service",
            pk_pem,
        )));
    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
