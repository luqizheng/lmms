use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;

use crate::{
    dto::{
        ApiResponse, CreateMenuConfigRequest, MenuConfigDetail, MenuConfigListResponse,
        UpdateMenuConfigRequest,
    },
    error::AppResult,
    service::MenuConfigService,
    state::AppState,
};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/get-async-routes", get(get_async_routes))
        .route("/pure/menu-configs", get(list_menu_configs).post(create_menu_config))
        .route(
            "/pure/menu-configs/{id}",
            get(get_menu_config)
                .put(update_menu_config)
                .delete(delete_menu_config),
        )
        .route("/pure/menu-configs/{id}/publish", post(publish_menu_config))
        .route("/pure/menu-configs/{id}/clone", post(clone_menu_config))
        .with_state(state)
}

async fn get_async_routes(State(state): State<AppState>) -> AppResult<Json<ApiResponse<Value>>> {
    let service = MenuConfigService::new(state);
    let routes = service.active_routes().await?;
    Ok(Json(ApiResponse::ok(routes)))
}

async fn list_menu_configs(
    State(state): State<AppState>,
) -> AppResult<Json<ApiResponse<MenuConfigListResponse>>> {
    let service = MenuConfigService::new(state);
    let result = service.list().await?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn create_menu_config(
    State(state): State<AppState>,
    Json(req): Json<CreateMenuConfigRequest>,
) -> AppResult<Json<ApiResponse<MenuConfigDetail>>> {
    let service = MenuConfigService::new(state);
    let result = service.create(req).await?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn get_menu_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<MenuConfigDetail>>> {
    let service = MenuConfigService::new(state);
    let result = service.get(&id).await?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn update_menu_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateMenuConfigRequest>,
) -> AppResult<Json<ApiResponse<MenuConfigDetail>>> {
    let service = MenuConfigService::new(state);
    let result = service.update(&id, req).await?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn publish_menu_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<MenuConfigDetail>>> {
    let service = MenuConfigService::new(state);
    let result = service.publish(&id).await?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn clone_menu_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<MenuConfigDetail>>> {
    let service = MenuConfigService::new(state);
    let result = service.clone_config(&id).await?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn delete_menu_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let service = MenuConfigService::new(state);
    service.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}
