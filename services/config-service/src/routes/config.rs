use axum::{extract::State, Json, extract::Path, extract::Query, http::StatusCode};
use crate::models::config_item::{ConfigItem, CreateConfigItem, PagedConfigItems, QueryConfigItems, UpdateConfigItem};
use crate::routes::AppState;

pub fn routes(state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/item", axum::routing::post(create_config_item))
        .route("/item", axum::routing::get(query_config_items))
        .route("/item/:id", axum::routing::get(get_config_item))
        .route("/item/:id", axum::routing::put(update_config_item))
        .route("/item/:id", axum::routing::delete(delete_config_item))
        .route("/export/:namespace_id", axum::routing::get(export_config))
        .route("/import/:namespace_id", axum::routing::post(import_config))
        .with_state(state)
}

async fn create_config_item(
    State(state): State<AppState>,
    Json(item): Json<CreateConfigItem>,
) -> Result<Json<ConfigItem>, (StatusCode, &'static str)> {
    let result = state.create_config_item(item).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "create failed"))?;
    Ok(Json(result))
}

async fn query_config_items(
    State(state): State<AppState>,
    Query(query): Query<QueryConfigItems>,
) -> Result<Json<PagedConfigItems>, (StatusCode, &'static str)> {
    let result = state.query_config_items(query).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "query failed"))?;
    Ok(Json(result))
}

async fn get_config_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ConfigItem>, (StatusCode, &'static str)> {
    let result = state.get_config_item(&id).await.map_err(|e| {
        if e.code() == "CFG_ITEM_1001" { (StatusCode::NOT_FOUND, "config item not found") } else { (StatusCode::INTERNAL_SERVER_ERROR, "get failed") }
    })?;
    Ok(Json(result))
}

async fn update_config_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(update): Json<UpdateConfigItem>,
) -> Result<Json<ConfigItem>, (StatusCode, &'static str)> {
    let result = state.update_config_item(&id, update).await.map_err(|e| {
        if e.code() == "CFG_ITEM_1001" { (StatusCode::NOT_FOUND, "config item not found") } else { (StatusCode::INTERNAL_SERVER_ERROR, "update failed") }
    })?;
    Ok(Json(result))
}

async fn delete_config_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, &'static str)> {
    state.delete_config_item(&id).await.map_err(|e| {
        if e.code() == "CFG_ITEM_1001" { (StatusCode::NOT_FOUND, "config item not found") } else { (StatusCode::INTERNAL_SERVER_ERROR, "delete failed") }
    })?;
    Ok(Json(serde_json::json!({ "success": true })))
}

async fn export_config(
    State(state): State<AppState>,
    Path(namespace_id): Path<String>,
) -> Result<String, (StatusCode, &'static str)> {
    let query = QueryConfigItems {
        namespace_id: Some(namespace_id),
        page: Some(1),
        size: Some(10000),
        key: None,
    };
    let result = state.query_config_items(query).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "export failed"))?;
    
    let mut data = std::collections::HashMap::new();
    for item in result.items {
        data.insert(item.key, item.value);
    }
    
    Ok(serde_json::to_string_pretty(&data).map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "serialize failed"))?)
}

async fn import_config(
    State(state): State<AppState>,
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
        let _ = state.create_config_item(item).await;
    }
    
    Ok(Json(serde_json::json!({ "success": true, "count": count })))
}
