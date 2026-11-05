use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use digua_jwt_ctx::Principal;
use serde_json::Value;

use crate::{
    dto::{
        ApiResponse, CreateMenuConfigRequest, MenuConfigDetail, MenuConfigListResponse,
        UpdateMenuConfigRequest,
    },
    error::{AppError, AppResult},
    service::MenuConfigService,
    state::AppState,
};

/// 2026-11-05 review (P0-8): menu-config writes define the entire frontend
/// dynamic-route surface — a published config redirects every pure-admin
/// shell. They previously accepted any caller without any JWT. Reads stay
/// open to any authenticated user; writes require an admin-class principal.
fn require_menu_admin(principal: &Principal) -> AppResult<()> {
    if principal.cross_tenant
        || principal.has_role("admin")
        || principal.has_role("platform_admin")
        || principal.has_role("tenant_admin")
    {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "menu config admin role required".to_string(),
        ))
    }
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/get-async-routes", get(get_async_routes))
        .route(
            "/pure/menu-configs",
            get(list_menu_configs).post(create_menu_config),
        )
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

#[utoipa::path(
    get,
    path = "/get-async-routes",
    summary = "Get active dynamic routes (sanitized for pure-admin)",
    responses(
        (status = 200, description = "Active dynamic routes (sanitized)", body = serde_json::Value),
    ),
    tag = "menu-configs"
)]
pub async fn get_async_routes(
    State(state): State<AppState>,
) -> AppResult<Json<ApiResponse<Value>>> {
    let service = MenuConfigService::new(state);
    let routes = service.active_routes().await?;
    Ok(Json(ApiResponse::ok(routes)))
}

#[utoipa::path(
    get,
    path = "/pure/menu-configs",
    summary = "List menu config versions",
    responses(
        (status = 200, description = "Menu config versions", body = MenuConfigListResponse),
    ),
    tag = "menu-configs"
)]
pub async fn list_menu_configs(
    State(state): State<AppState>,
) -> AppResult<Json<ApiResponse<MenuConfigListResponse>>> {
    let service = MenuConfigService::new(state);
    let result = service.list().await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(
    post,
    path = "/pure/menu-configs",
    summary = "Create menu config draft",
    request_body = CreateMenuConfigRequest,
    responses(
        (status = 200, description = "Draft created", body = MenuConfigDetail),
    ),
    tag = "menu-configs"
)]
pub async fn create_menu_config(
    State(state): State<AppState>,
    principal: Principal,
    Json(req): Json<CreateMenuConfigRequest>,
) -> AppResult<Json<ApiResponse<MenuConfigDetail>>> {
    require_menu_admin(&principal)?;
    let service = MenuConfigService::new(state);
    let result = service.create(req).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(
    get,
    path = "/pure/menu-configs/{id}",
    summary = "Get menu config by id",
    params(("id" = String, Path, description = "Menu config id")),
    responses(
        (status = 200, description = "Menu config detail", body = MenuConfigDetail),
        (status = 404, description = "Not found"),
    ),
    tag = "menu-configs"
)]
pub async fn get_menu_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<MenuConfigDetail>>> {
    let service = MenuConfigService::new(state);
    let result = service.get(&id).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(
    put,
    path = "/pure/menu-configs/{id}",
    summary = "Update menu config draft (requires etag)",
    params(("id" = String, Path, description = "Menu config id")),
    request_body = UpdateMenuConfigRequest,
    responses(
        (status = 200, description = "Updated draft", body = MenuConfigDetail),
        (status = 400, description = "Validation failed (e.g. missing if_match_etag)"),
        (status = 409, description = "Etag conflict or status not draft"),
    ),
    tag = "menu-configs"
)]
pub async fn update_menu_config(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<String>,
    Json(req): Json<UpdateMenuConfigRequest>,
) -> AppResult<Json<ApiResponse<MenuConfigDetail>>> {
    require_menu_admin(&principal)?;
    let service = MenuConfigService::new(state);
    let result = service.update(&id, req).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(
    post,
    path = "/pure/menu-configs/{id}/publish",
    summary = "Publish menu config (draft → published)",
    params(("id" = String, Path, description = "Menu config id")),
    responses(
        (status = 200, description = "Published", body = MenuConfigDetail),
    ),
    tag = "menu-configs"
)]
pub async fn publish_menu_config(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<MenuConfigDetail>>> {
    require_menu_admin(&principal)?;
    let service = MenuConfigService::new(state);
    let result = service.publish(&id).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(
    post,
    path = "/pure/menu-configs/{id}/clone",
    summary = "Clone menu config",
    params(("id" = String, Path, description = "Menu config id")),
    responses(
        (status = 200, description = "Cloned draft", body = MenuConfigDetail),
    ),
    tag = "menu-configs"
)]
pub async fn clone_menu_config(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<MenuConfigDetail>>> {
    require_menu_admin(&principal)?;
    let service = MenuConfigService::new(state);
    let result = service.clone_config(&id).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(
    delete,
    path = "/pure/menu-configs/{id}",
    summary = "Delete menu config",
    params(("id" = String, Path, description = "Menu config id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 409, description = "Cannot delete published config"),
    ),
    tag = "menu-configs"
)]
pub async fn delete_menu_config(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    require_menu_admin(&principal)?;
    let service = MenuConfigService::new(state);
    service.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}
