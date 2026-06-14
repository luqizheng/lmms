use axum::{extract::State, Json, extract::Path, extract::Query};
use crate::error::AppResult;
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
) -> AppResult<Json<ConfigItem>> {
    let result = state.create_config_item(item).await?;
    Ok(Json(result))
}

async fn query_config_items(
    State(state): State<AppState>,
    Query(query): Query<QueryConfigItems>,
) -> AppResult<Json<PagedConfigItems>> {
    let result = state.query_config_items(query).await?;
    Ok(Json(result))
}

async fn get_config_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ConfigItem>> {
    let result = state.get_config_item(&id).await?;
    Ok(Json(result))
}

async fn update_config_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(update): Json<UpdateConfigItem>,
) -> AppResult<Json<ConfigItem>> {
    let result = state.update_config_item(&id, update).await?;
    Ok(Json(result))
}

async fn delete_config_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    state.delete_config_item(&id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

async fn export_config(
    State(state): State<AppState>,
    Path(namespace_id): Path<String>,
) -> AppResult<String> {
    let query = QueryConfigItems {
        namespace_id: Some(namespace_id),
        page: Some(1),
        size: Some(10000),
        key: None,
    };
    let result = state.query_config_items(query).await?;
    
    let mut data = std::collections::HashMap::new();
    for item in result.items {
        data.insert(item.key, item.value);
    }
    
    Ok(serde_json::to_string_pretty(&data)?)
}

async fn import_config(
    State(state): State<AppState>,
    Path(namespace_id): Path<String>,
    body: String,
) -> AppResult<Json<serde_json::Value>> {
    let configs: std::collections::HashMap<String, String> = serde_json::from_str(&body)?;
    
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
    
    Ok(Json(serde_json::json!({ "success": true, "count": configs.len() })))
}
