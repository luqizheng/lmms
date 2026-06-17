use axum::{Router, routing::{get, post, put, delete}, extract::{Json, Path, State}};
use crate::{state::AppState, dto::{CreateMenuRequest, UpdateMenuRequest, MenuResponse, MenuListResponse}, service::MenuService};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/menus", get(list_menus).post(create_menu))
        .route("/iam/menus/{menu_id}", get(get_menu).put(update_menu).delete(delete_menu))
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/iam/menus",
    responses((status = 200, body = MenuListResponse)),
    security(("bearer_auth" = [])),
    tag = "menus"
)]
pub async fn list_menus(
    State(state): State<AppState>,
) -> Result<Json<MenuListResponse>, (axum::http::StatusCode, &'static str)> {
    let service = MenuService::new(state);
    let response = service.list().await
        .map_err(|_| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/iam/menus",
    request_body = CreateMenuRequest,
    responses(
        (status = 200, body = MenuResponse),
        (status = 409, description = "duplicate menu"),
    ),
    security(("bearer_auth" = [])),
    tag = "menus"
)]
pub async fn create_menu(
    State(state): State<AppState>,
    Json(req): Json<CreateMenuRequest>,
) -> Result<Json<MenuResponse>, (axum::http::StatusCode, &'static str)> {
    let service = MenuService::new(state);
    let response = service.create(req).await.map_err(|e| {
        match e {
            crate::error::IamError::DuplicateMenu => (axum::http::StatusCode::CONFLICT, "duplicate menu"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/iam/menus/{menu_id}",
    params(("menu_id" = String, Path,)),
    responses(
        (status = 200, body = MenuResponse),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "menus"
)]
pub async fn get_menu(
    State(state): State<AppState>,
    Path(menu_id): Path<String>,
) -> Result<Json<MenuResponse>, (axum::http::StatusCode, &'static str)> {
    let service = MenuService::new(state);
    let response = service.get(&menu_id).await.map_err(|e| {
        match e {
            crate::error::IamError::MenuNotFound => (axum::http::StatusCode::NOT_FOUND, "menu not found"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/iam/menus/{menu_id}",
    params(("menu_id" = String, Path,)),
    request_body = UpdateMenuRequest,
    responses(
        (status = 200, body = MenuResponse),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "menus"
)]
pub async fn update_menu(
    State(state): State<AppState>,
    Path(menu_id): Path<String>,
    Json(req): Json<UpdateMenuRequest>,
) -> Result<Json<MenuResponse>, (axum::http::StatusCode, &'static str)> {
    let service = MenuService::new(state);
    let response = service.update(&menu_id, req).await.map_err(|e| {
        match e {
            crate::error::IamError::MenuNotFound => (axum::http::StatusCode::NOT_FOUND, "menu not found"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/iam/menus/{menu_id}",
    params(("menu_id" = String, Path,)),
    responses(
        (status = 204, description = "deleted"),
        (status = 404, description = "not found"),
        (status = 409, description = "conflict"),
    ),
    security(("bearer_auth" = [])),
    tag = "menus"
)]
pub async fn delete_menu(
    State(state): State<AppState>,
    Path(menu_id): Path<String>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, &'static str)> {
    let service = MenuService::new(state);
    service.delete(&menu_id).await.map_err(|e| {
        match e {
            crate::error::IamError::MenuNotFound => (axum::http::StatusCode::NOT_FOUND, "menu not found"),
            crate::error::IamError::Conflict(_) => (axum::http::StatusCode::CONFLICT, "conflict"),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        }
    })?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}