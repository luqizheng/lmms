use crate::models::namespace::{
    CreateNamespace, Namespace, PagedNamespaces, QueryNamespaces, UpdateNamespace,
};
use crate::routes::{config::require_config_perm, AppState};
use axum::{extract::Path, extract::Query, extract::State, http::StatusCode, Json};
use digua_authz::perms::perms;
use digua_jwt_ctx::{Principal, TenantScope};

pub fn routes(state: AppState) -> axum::Router<AppState> {
    axum::Router::new()
        .route("/namespace", axum::routing::post(create_namespace))
        .route("/namespace", axum::routing::get(query_namespaces))
        .route("/namespace/{id}", axum::routing::get(get_namespace))
        .route("/namespace/{id}", axum::routing::put(update_namespace))
        .route("/namespace/{id}", axum::routing::delete(delete_namespace))
        .with_state(state)
}

#[utoipa::path(
    post,
    path = "/config/namespace",
    summary = "Create config namespace",
    request_body = CreateNamespace,
    responses(
        (status = 200, description = "Namespace created", body = Namespace),
    ),
    tag = "namespace"
)]
// check-tenant-scope: delegated (tenant_id from scope applied by service::create_namespace)
pub async fn create_namespace(
    State(state): State<AppState>,
    principal: Principal,
    scope: TenantScope,
    Json(ns): Json<CreateNamespace>,
) -> Result<Json<Namespace>, (StatusCode, &'static str)> {
    // 2026-11-05 review (P0-6): namespace writes were unguarded too.
    require_config_perm(&principal, perms::CFG_NAMESPACE_CREATE)?;
    let result = state
        .create_namespace(ns, &scope)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "create failed"))?;
    Ok(Json(result))
}

#[utoipa::path(
    get,
    path = "/config/namespace",
    summary = "List config namespaces",
    params(QueryNamespaces),
    responses(
        (status = 200, description = "Paged namespaces", body = PagedNamespaces),
    ),
    tag = "namespace"
)]
// check-tenant-scope: delegated (and_tenant applied by service::query_namespaces)
pub async fn query_namespaces(
    State(state): State<AppState>,
    Query(query): Query<QueryNamespaces>,
    scope: TenantScope,
) -> Result<Json<PagedNamespaces>, (StatusCode, &'static str)> {
    let result = state
        .query_namespaces(query, &scope)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "query failed"))?;
    Ok(Json(result))
}

#[utoipa::path(
    get,
    path = "/config/namespace/{id}",
    summary = "Get config namespace by id",
    params(("id" = String, Path, description = "Namespace id")),
    responses(
        (status = 200, description = "Namespace", body = Namespace),
        (status = 404, description = "Not found"),
    ),
    tag = "namespace"
)]
// check-tenant-scope: delegated (where_id + and_tenant applied by service::get_namespace)
pub async fn get_namespace(
    State(state): State<AppState>,
    scope: TenantScope,
    Path(id): Path<String>,
) -> Result<Json<Namespace>, (StatusCode, &'static str)> {
    let result = state.get_namespace(&id, &scope).await.map_err(|e| {
        if e.code() == "CFG_NS_2001" {
            (StatusCode::NOT_FOUND, "namespace not found")
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, "get failed")
        }
    })?;
    Ok(Json(result))
}

#[utoipa::path(
    put,
    path = "/config/namespace/{id}",
    summary = "Update config namespace",
    params(("id" = String, Path, description = "Namespace id")),
    request_body = UpdateNamespace,
    responses(
        (status = 200, description = "Updated namespace", body = Namespace),
        (status = 404, description = "Not found"),
    ),
    tag = "namespace"
)]
// check-tenant-scope: delegated (where_id + and_tenant applied by service::update_namespace)
pub async fn update_namespace(
    State(state): State<AppState>,
    principal: Principal,
    scope: TenantScope,
    Path(id): Path<String>,
    Json(update): Json<UpdateNamespace>,
) -> Result<Json<Namespace>, (StatusCode, &'static str)> {
    require_config_perm(&principal, perms::CFG_NAMESPACE_UPDATE)?;
    let result = state
        .update_namespace(&id, update, &scope)
        .await
        .map_err(|e| {
            if e.code() == "CFG_NS_2001" {
                (StatusCode::NOT_FOUND, "namespace not found")
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, "update failed")
            }
        })?;
    Ok(Json(result))
}

#[utoipa::path(
    delete,
    path = "/config/namespace/{id}",
    summary = "Delete config namespace",
    params(("id" = String, Path, description = "Namespace id")),
    responses(
        (status = 200, description = "Deleted", body = serde_json::Value),
        (status = 404, description = "Not found"),
    ),
    tag = "namespace"
)]
// check-tenant-scope: delegated (where_id + and_tenant applied by service::delete_namespace)
pub async fn delete_namespace(
    State(state): State<AppState>,
    principal: Principal,
    scope: TenantScope,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, &'static str)> {
    require_config_perm(&principal, perms::CFG_NAMESPACE_DELETE)?;
    state.delete_namespace(&id, &scope).await.map_err(|e| {
        if e.code() == "CFG_NS_2001" {
            (StatusCode::NOT_FOUND, "namespace not found")
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, "delete failed")
        }
    })?;
    Ok(Json(serde_json::json!({ "success": true })))
}
