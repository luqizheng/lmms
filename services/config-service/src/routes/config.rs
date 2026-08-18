use axum::{extract::State, Json, extract::Path, extract::Query, http::StatusCode};
use digua_jwt_ctx::{Principal, TenantScope};
use crate::models::config_item::{ConfigItem, CreateConfigItem, PagedConfigItems, QueryConfigItems, UpdateConfigItem};
use crate::routes::AppState;

/// 掩码的常量 —— 与前端 `api/config/masking.ts` `MASK` 保持一致
const MASK: &str = "********";

const REVEAL_PERM: &str = "CFG_CONFIG_REVEAL";

/// is_secret 标记为 true 且 caller 不持有 CFG_CONFIG_REVEAL 时，
/// 把 value 置为掩码并标 value_masked=true。
fn apply_reveal_policy(item: &mut ConfigItem, principal: &Principal) {
    if item.is_secret && !principal.has_permission(REVEAL_PERM) {
        item.value = MASK.to_string();
        item.value_masked = true;
    }
}

pub fn routes(state: AppState) -> axum::Router<AppState> {
    axum::Router::new()
        .route("/item", axum::routing::post(create_config_item))
        .route("/item", axum::routing::get(query_config_items))
        .route("/item/{id}", axum::routing::get(get_config_item))
        .route("/item/{id}", axum::routing::put(update_config_item))
        .route("/item/{id}", axum::routing::delete(delete_config_item))
        .route("/export/{namespace_id}", axum::routing::get(export_config))
        .route("/import/{namespace_id}", axum::routing::post(import_config))
        .with_state(state)
}

#[utoipa::path(
    post,
    path = "/config/item",
    summary = "Create config item",
    request_body = CreateConfigItem,
    responses(
        (status = 200, description = "Config item created", body = ConfigItem),
    ),
    tag = "config-item"
)]
// check-tenant-scope: delegated (namespace ownership enforced by service::create_config_item via where_id+and_tenant)
pub async fn create_config_item(
    State(state): State<AppState>,
    principal: Principal,
    scope: TenantScope,
    Json(item): Json<CreateConfigItem>,
) -> Result<Json<ConfigItem>, (StatusCode, &'static str)> {
    let mut result = state.create_config_item(item, &scope).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "create failed"))?;
    apply_reveal_policy(&mut result, &principal);
    Ok(Json(result))
}

#[utoipa::path(
    get,
    path = "/config/item",
    summary = "List config items",
    params(QueryConfigItems),
    responses(
        (status = 200, description = "Paged config items", body = PagedConfigItems),
    ),
    tag = "config-item"
)]
// check-tenant-scope: delegated (JOIN cfg_namespaces tenant filter applied by service::query_config_items)
pub async fn query_config_items(
    State(state): State<AppState>,
    Query(query): Query<QueryConfigItems>,
    principal: Principal,
    scope: TenantScope,
) -> Result<Json<PagedConfigItems>, (StatusCode, &'static str)> {
    let mut result = state.query_config_items(query, &scope).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "query failed"))?;
    for item in result.items.iter_mut() {
        apply_reveal_policy(item, &principal);
    }
    Ok(Json(result))
}

#[utoipa::path(
    get,
    path = "/config/item/{id}",
    summary = "Get config item by id",
    params(("id" = String, Path, description = "Config item id")),
    responses(
        (status = 200, description = "Config item", body = ConfigItem),
        (status = 404, description = "Not found"),
    ),
    tag = "config-item"
)]
// check-tenant-scope: delegated (JOIN cfg_namespaces tenant filter applied by service::get_config_item)
pub async fn get_config_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
    principal: Principal,
    scope: TenantScope,
) -> Result<Json<ConfigItem>, (StatusCode, &'static str)> {
    let mut result = state.get_config_item(&id, &scope).await.map_err(|e| {
        if e.code() == "CFG_ITEM_1001" { (StatusCode::NOT_FOUND, "config item not found") } else { (StatusCode::INTERNAL_SERVER_ERROR, "get failed") }
    })?;
    apply_reveal_policy(&mut result, &principal);
    Ok(Json(result))
}

#[utoipa::path(
    put,
    path = "/config/item/{id}",
    summary = "Update config item",
    params(("id" = String, Path, description = "Config item id")),
    request_body = UpdateConfigItem,
    responses(
        (status = 200, description = "Updated config item", body = ConfigItem),
        (status = 404, description = "Not found"),
    ),
    tag = "config-item"
)]
// check-tenant-scope: delegated (JOIN cfg_namespaces tenant filter applied by service::update_config_item)
pub async fn update_config_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
    principal: Principal,
    scope: TenantScope,
    Json(update): Json<UpdateConfigItem>,
) -> Result<Json<ConfigItem>, (StatusCode, &'static str)> {
    let mut result = state.update_config_item(&id, update, &scope).await.map_err(|e| {
        if e.code() == "CFG_ITEM_1001" { (StatusCode::NOT_FOUND, "config item not found") } else { (StatusCode::INTERNAL_SERVER_ERROR, "update failed") }
    })?;
    apply_reveal_policy(&mut result, &principal);
    Ok(Json(result))
}

#[utoipa::path(
    delete,
    path = "/config/item/{id}",
    summary = "Delete config item",
    params(("id" = String, Path, description = "Config item id")),
    responses(
        (status = 200, description = "Deleted", body = serde_json::Value),
        (status = 404, description = "Not found"),
    ),
    tag = "config-item"
)]
// check-tenant-scope: delegated (JOIN cfg_namespaces tenant filter applied by service::delete_config_item)
pub async fn delete_config_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
    scope: TenantScope,
) -> Result<Json<serde_json::Value>, (StatusCode, &'static str)> {
    state.delete_config_item(&id, &scope).await.map_err(|e| {
        if e.code() == "CFG_ITEM_1001" { (StatusCode::NOT_FOUND, "config item not found") } else { (StatusCode::INTERNAL_SERVER_ERROR, "delete failed") }
    })?;
    Ok(Json(serde_json::json!({ "success": true })))
}

#[utoipa::path(
    get,
    path = "/config/export/{namespace_id}",
    summary = "Export config as JSON",
    params(("namespace_id" = String, Path, description = "Namespace id")),
    responses(
        (status = 200, description = "Exported config as JSON string", body = String),
    ),
    tag = "config-item"
)]
// check-tenant-scope: delegated (JOIN cfg_namespaces tenant filter applied by service::query_config_items)
pub async fn export_config(
    State(state): State<AppState>,
    Path(namespace_id): Path<String>,
    principal: Principal,
    scope: TenantScope,
) -> Result<String, (StatusCode, &'static str)> {
    let query = QueryConfigItems {
        namespace_id: Some(namespace_id),
        page: Some(1),
        size: Some(10000),
        key: None,
    };
    let mut result = state.query_config_items(query, &scope).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "export failed"))?;

    let mut data = std::collections::HashMap::new();
    for item in result.items.iter_mut() {
        apply_reveal_policy(item, &principal);
        data.insert(item.key.clone(), item.value.clone());
    }

    Ok(serde_json::to_string_pretty(&data).map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "serialize failed"))?)
}

#[utoipa::path(
    post,
    path = "/config/import/{namespace_id}",
    summary = "Import config from JSON",
    params(("namespace_id" = String, Path, description = "Namespace id")),
    request_body = String,
    responses(
        (status = 200, description = "Import result", body = serde_json::Value),
        (status = 400, description = "Invalid JSON body"),
    ),
    tag = "config-item"
)]
// check-tenant-scope: delegated (namespace ownership enforced by service::create_config_item via where_id+and_tenant)
pub async fn import_config(
    State(state): State<AppState>,
    _principal: Principal,
    scope: TenantScope,
    Path(namespace_id): Path<String>,
    body: String,
) -> Result<Json<serde_json::Value>, (StatusCode, &'static str)> {
    let configs: std::collections::HashMap<String, String> = serde_json::from_str(&body).map_err(|_| (StatusCode::BAD_REQUEST, "invalid json"))?;
    let count = configs.len();

    for (key, value) in configs {
        let item = CreateConfigItem {
            namespace_id: namespace_id.clone(),
            key,
            value,
            value_type: "string".to_string(),
            is_secret: false,
            description: None,
        };
        let _ = state.create_config_item(item, &scope).await;
    }

    Ok(Json(serde_json::json!({ "success": true, "count": count })))
}

#[cfg(test)]
mod tests {
    //! 5 个 handler 调用点的 reveal policy 契约测试：
    //! create / query / get / update / export。已知边界：本测试直接调函数，
    //! 不跑真实 handler 路径，因此**不能**捕获「有人删掉 handler 内的
    //! apply_reveal_policy 行」。完整端到端（mock AppState + tower::oneshot）
    //! 集成测试 Phase 10 后用 testcontainers 完成。
    //! 函数本身的 baseline 测试见 `tests/reveal_policy_test.rs`。

    use super::*;
    use chrono::Utc;
    use digua_jwt_ctx::Principal;
    use std::collections::HashSet;

    fn make_item(is_secret: bool, value: &str) -> ConfigItem {
        ConfigItem {
            id: "i1".into(),
            namespace_id: "ns1".into(),
            key: "k".into(),
            value: value.into(),
            value_type: "string".into(),
            is_secret,
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            value_masked: false,
        }
    }

    fn make_principal(perms: &[&str]) -> Principal {
        Principal {
            jti: "test".into(),
            user_id: "u1".into(),
            user_code: "u1".into(),
            tenant_id: 1,
            cross_tenant: false,
            dept_id: None,
            roles: vec!["admin".into()],
            permissions: perms.iter().map(|s| s.to_string()).collect::<HashSet<_>>(),
        }
    }

    #[test]
    fn handler_create_masks_secret_when_caller_lacks_perm() {
        let mut item = make_item(true, "raw-secret");
        let principal = make_principal(&["CFG_CONFIG_CREATE"]);
        apply_reveal_policy(&mut item, &principal);
        assert_eq!(item.value, "********");
        assert!(item.value_masked);
    }

    #[test]
    fn handler_query_masks_each_secret_in_list_when_caller_lacks_perm() {
        let principal = make_principal(&["CFG_CONFIG_READ"]);
        let mut items = vec![
            make_item(true, "secret-a"),
            make_item(false, "plain-b"),
            make_item(true, "secret-c"),
        ];
        for item in items.iter_mut() {
            apply_reveal_policy(item, &principal);
        }
        assert_eq!(items[0].value, "********");
        assert!(items[0].value_masked);
        assert_eq!(items[1].value, "plain-b");
        assert!(!items[1].value_masked);
        assert_eq!(items[2].value, "********");
        assert!(items[2].value_masked);
    }

    #[test]
    fn handler_get_masks_secret_when_caller_lacks_perm() {
        let mut item = make_item(true, "raw-secret");
        let principal = make_principal(&["CFG_CONFIG_READ"]);
        apply_reveal_policy(&mut item, &principal);
        assert_eq!(item.value, "********");
        assert!(item.value_masked);
    }

    #[test]
    fn handler_update_masks_secret_when_caller_lacks_perm() {
        let mut item = make_item(true, "new-value");
        let principal = make_principal(&["CFG_CONFIG_UPDATE"]);
        apply_reveal_policy(&mut item, &principal);
        assert_eq!(item.value, "********");
        assert!(item.value_masked);
    }

    #[test]
    fn handler_export_masks_all_secrets_when_caller_lacks_perm() {
        let principal = make_principal(&["CFG_NAMESPACE_EXPORT"]);
        let mut items = vec![
            make_item(true, "secret-1"),
            make_item(true, "secret-2"),
            make_item(false, "plain-3"),
        ];
        for item in items.iter_mut() {
            apply_reveal_policy(item, &principal);
        }
        assert_eq!(items[0].value, "********");
        assert!(items[0].value_masked);
        assert_eq!(items[1].value, "********");
        assert!(items[1].value_masked);
        assert_eq!(items[2].value, "plain-3");
        assert!(!items[2].value_masked);
    }

    #[test]
    fn reveal_perm_passes_through_all_handler_paths() {
        let principal = make_principal(&["CFG_CONFIG_REVEAL"]);
        for value in ["secret-x", "secret-y"] {
            let mut item = make_item(true, value);
            apply_reveal_policy(&mut item, &principal);
            assert_eq!(item.value, value);
            assert!(!item.value_masked);
        }
    }
}
