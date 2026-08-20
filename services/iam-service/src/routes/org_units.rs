use crate::{
    dto::{AssignUsersRequest, CreateOrgUnitRequest, OrgUnitResponse, UpdateOrgUnitRequest},
    service::OrgService,
    state::AppState,
};
use axum::{
    extract::{Json, Path, State},
    routing::{delete, get, post, put},
    Router,
};
use digua_jwt_ctx::TenantScope;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/org-units", get(list_org_units).post(create_org_unit))
        .route(
            "/iam/org-units/{org_unit_id}",
            get(get_org_unit)
                .put(update_org_unit)
                .delete(delete_org_unit),
        )
        .route("/iam/org-units/{org_unit_id}/users", put(assign_users))
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/iam/org-units",
    responses((status = 200, body = Vec<OrgUnitResponse>)),
    security(("bearer_auth" = [])),
    tag = "org_units"
)]
// check-tenant-scope: delegated
async fn list_org_units(
    State(state): State<AppState>,
    scope: TenantScope,
) -> Result<Json<Vec<OrgUnitResponse>>, (axum::http::StatusCode, &'static str)> {
    let service = OrgService::new(state);
    let response = service.list(&scope).await.map_err(|_| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "internal error",
        )
    })?;
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/iam/org-units",
    request_body = CreateOrgUnitRequest,
    responses(
        (status = 200, body = OrgUnitResponse),
        (status = 409, description = "duplicate org unit"),
    ),
    security(("bearer_auth" = [])),
    tag = "org_units"
)]
// check-tenant-scope: delegated
async fn create_org_unit(
    State(state): State<AppState>,
    scope: TenantScope,
    Json(req): Json<CreateOrgUnitRequest>,
) -> Result<Json<OrgUnitResponse>, (axum::http::StatusCode, &'static str)> {
    let service = OrgService::new(state);
    let response = service.create(req, &scope).await.map_err(|e| match e {
        crate::error::IamError::DuplicateOrgUnit => {
            (axum::http::StatusCode::CONFLICT, "duplicate org unit")
        }
        _ => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "internal error",
        ),
    })?;
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/iam/org-units/{org_unit_id}",
    params(("org_unit_id" = String, Path, description = "Org unit ID")),
    responses(
        (status = 200, body = OrgUnitResponse),
        (status = 404, description = "org unit not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "org_units"
)]
// check-tenant-scope: delegated
async fn get_org_unit(
    State(state): State<AppState>,
    Path(org_unit_id): Path<String>,
    scope: TenantScope,
) -> Result<Json<OrgUnitResponse>, (axum::http::StatusCode, &'static str)> {
    let service = OrgService::new(state);
    let response = service
        .get(&org_unit_id, &scope)
        .await
        .map_err(|e| match e {
            crate::error::IamError::OrgUnitNotFound => {
                (axum::http::StatusCode::NOT_FOUND, "org unit not found")
            }
            _ => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal error",
            ),
        })?;
    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/iam/org-units/{org_unit_id}",
    params(("org_unit_id" = String, Path, description = "Org unit ID")),
    request_body = UpdateOrgUnitRequest,
    responses(
        (status = 200, body = OrgUnitResponse),
        (status = 404, description = "org unit not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "org_units"
)]
// check-tenant-scope: delegated
async fn update_org_unit(
    State(state): State<AppState>,
    Path(org_unit_id): Path<String>,
    scope: TenantScope,
    Json(req): Json<UpdateOrgUnitRequest>,
) -> Result<Json<OrgUnitResponse>, (axum::http::StatusCode, &'static str)> {
    let service = OrgService::new(state);
    let response = service
        .update(&org_unit_id, req, &scope)
        .await
        .map_err(|e| match e {
            crate::error::IamError::OrgUnitNotFound => {
                (axum::http::StatusCode::NOT_FOUND, "org unit not found")
            }
            _ => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal error",
            ),
        })?;
    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/iam/org-units/{org_unit_id}",
    params(("org_unit_id" = String, Path, description = "Org unit ID")),
    responses(
        (status = 204, description = "org unit deleted"),
        (status = 404, description = "org unit not found"),
        (status = 409, description = "conflict"),
    ),
    security(("bearer_auth" = [])),
    tag = "org_units"
)]
// check-tenant-scope: delegated
async fn delete_org_unit(
    State(state): State<AppState>,
    Path(org_unit_id): Path<String>,
    scope: TenantScope,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, &'static str)> {
    let service = OrgService::new(state);
    service
        .delete(&org_unit_id, &scope)
        .await
        .map_err(|e| match e {
            crate::error::IamError::OrgUnitNotFound => {
                (axum::http::StatusCode::NOT_FOUND, "org unit not found")
            }
            crate::error::IamError::Conflict(_) => (axum::http::StatusCode::CONFLICT, "conflict"),
            _ => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal error",
            ),
        })?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/iam/org-units/{org_unit_id}/users",
    params(("org_unit_id" = String, Path, description = "Org unit ID")),
    request_body = AssignUsersRequest,
    responses((status = 204, description = "users assigned")),
    security(("bearer_auth" = [])),
    tag = "org_units"
)]
// check-tenant-scope: delegated
async fn assign_users(
    State(state): State<AppState>,
    Path(org_unit_id): Path<String>,
    scope: TenantScope,
    Json(req): Json<AssignUsersRequest>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, &'static str)> {
    let service = OrgService::new(state);
    service
        .assign_users(&org_unit_id, req, &scope)
        .await
        .map_err(|_| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal error",
            )
        })?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
