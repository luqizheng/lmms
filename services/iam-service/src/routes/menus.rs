use axum::{Router, routing::{get, post, put, delete}, extract::{Json, Path, State}};
use crate::{state::AppState, dto::{CreateMenuRequest, UpdateMenuRequest, MenuResponse, MenuListResponse}, service::MenuService};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/menus", get(list_menus).post(create_menu))
        .route("/iam/menus/:menu_id", get(get_menu).put(update_menu).delete(delete_menu))
        .with_state(state)
}

async fn list_menus(
    State(state): State<AppState>,
) -> Result<Json<MenuListResponse>, axum::http::StatusCode> {
    let service = MenuService::new(state);
    let response = service.list().await.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(response))
}

async fn create_menu(
    State(state): State<AppState>,
    Json(req): Json<CreateMenuRequest>,
) -> Result<Json<MenuResponse>, axum::http::StatusCode> {
    let service = MenuService::new(state);
    let response = service.create(req).await.map_err(|e| {
        match e {
            crate::error::IamError::DuplicateMenu => axum::http::StatusCode::CONFLICT,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn get_menu(
    State(state): State<AppState>,
    Path(menu_id): Path<String>,
) -> Result<Json<MenuResponse>, axum::http::StatusCode> {
    let service = MenuService::new(state);
    let response = service.get(&menu_id).await.map_err(|e| {
        match e {
            crate::error::IamError::MenuNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn update_menu(
    State(state): State<AppState>,
    Path(menu_id): Path<String>,
    Json(req): Json<UpdateMenuRequest>,
) -> Result<Json<MenuResponse>, axum::http::StatusCode> {
    let service = MenuService::new(state);
    let response = service.update(&menu_id, req).await.map_err(|e| {
        match e {
            crate::error::IamError::MenuNotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(Json(response))
}

async fn delete_menu(
    State(state): State<AppState>,
    Path(menu_id): Path<String>,
) -> Result<(), axum::http::StatusCode> {
    let service = MenuService::new(state);
    service.delete(&menu_id).await.map_err(|e| {
        match e {
            crate::error::IamError::MenuNotFound => axum::http::StatusCode::NOT_FOUND,
            crate::error::IamError::Conflict(_) => axum::http::StatusCode::CONFLICT,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;
    Ok(())
}
