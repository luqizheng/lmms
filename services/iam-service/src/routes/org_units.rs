use axum::{Router, routing::{get, post, put, delete}, extract::{Json, Path, State}};
use crate::{state::AppState, dto::{CreateOrgUnitRequest, UpdateOrgUnitRequest, OrgUnitResponse, AssignUsersRequest}, service::OrgService};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/org-units", get(list_org_units).post(create_org_unit))
        .route("/iam/org-units/:org_unit_id", get(get_org_unit).put(update_org_unit).delete(delete_org_unit))
        .route("/iam/org-units/:org_unit_id/users", put(assign_users))
        .with_state(state)
}

async fn list_org_units(
    State(state): State<AppState>,
) -> Result<Json<Vec<OrgUnitResponse>>, axum::http::StatusCode> {
    let service = OrgService::new(state);
    let response = service.list().await.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(response))
}

async fn create_org_unit(
    State(state): State<AppState>,
    Json(req): Json<CreateOrgUnitRequest>,
) -> Result<Json<OrgUnitResponse>, axum::http::StatusCode> {
    let service = OrgService::new(state);
    let response = service.create(req).await.map_err(|e| {
        match e {
            crate::error::IamError::DuplicateOrgUnit => axum::http::StatusCode::CONFLICT,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn get_org_unit(
    State(state): State<AppState>,
    Path(org_unit_id): Path<String>,
) -> Result<Json<OrgUnitResponse>, axum::http::StatusCode> {
    let service = OrgService::new(state);
    let response = service.get(&org_unit_id).await.map_err(|e| {
        match e {
            crate::error::IamError::OrgUnitNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn update_org_unit(
    State(state): State<AppState>,
    Path(org_unit_id): Path<String>,
    Json(req): Json<UpdateOrgUnitRequest>,
) -> Result<Json<OrgUnitResponse>, axum::http::StatusCode> {
    let service = OrgService::new(state);
    let response = service.update(&org_unit_id, req).await.map_err(|e| {
        match e {
            crate::error::IamError::OrgUnitNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn delete_org_unit(
    State(state): State<AppState>,
    Path(org_unit_id): Path<String>,
) -> Result<(), axum::http::StatusCode> {
    let service = OrgService::new(state);
    service.delete(&org_unit_id).await.map_err(|e| {
        match e {
            crate::error::IamError::OrgUnitNotFound => axum::http::StatusCode::NOT_FOUND,
            crate::error::IamError::Conflict(_) => axum::http::StatusCode::CONFLICT,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(())
}

async fn assign_users(
    State(state): State<AppState>,
    Path(org_unit_id): Path<String>,
    Json(req): Json<AssignUsersRequest>,
) -> Result<(), axum::http::StatusCode> {
    let service = OrgService::new(state);
    service.assign_users(&org_unit_id, req).await.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}
