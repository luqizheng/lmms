use axum::{
    extract::{Json, State},
    http::StatusCode,
    routing::post,
    Json as AxumJson, Router,
};
use validator::Validate;

use crate::{
    dto::{CreateTenantRequest, CreateTenantResponse},
    error::IamError,
    service::TenantService,
    state::AppState,
};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/tenants", post(register_tenant))
        .with_state(state)
}

#[utoipa::path(
    post,
    path = "/iam/tenants",
    request_body = CreateTenantRequest,
    responses(
        (status = 201, body = CreateTenantResponse),
        (status = 400, description = "invalid request"),
        (status = 409, description = "tenant slug already exists"),
    ),
    tag = "tenants"
)]
pub async fn register_tenant(
    State(state): State<AppState>,
    Json(req): Json<CreateTenantRequest>,
) -> Result<(StatusCode, AxumJson<CreateTenantResponse>), (StatusCode, &'static str)> {
    req.validate()
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid request"))?;
    let svc = TenantService::new(state);
    let resp = svc.register(req).await.map_err(|e| {
        tracing::error!("register tenant failed: {:?}", e);
        match e {
            IamError::DuplicateTenant => (StatusCode::CONFLICT, "tenant slug already exists"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok((StatusCode::CREATED, AxumJson(resp)))
}
