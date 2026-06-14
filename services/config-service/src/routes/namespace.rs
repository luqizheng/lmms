use axum::{extract::State, Json, extract::Path, extract::Query};
use crate::error::AppResult;
use crate::models::namespace::{CreateNamespace, Namespace, PagedNamespaces, QueryNamespaces, UpdateNamespace};
use crate::routes::AppState;

pub fn routes(state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/namespace", axum::routing::post(create_namespace))
        .route("/namespace", axum::routing::get(query_namespaces))
        .route("/namespace/:id", axum::routing::get(get_namespace))
        .route("/namespace/:id", axum::routing::put(update_namespace))
        .route("/namespace/:id", axum::routing::delete(delete_namespace))
        .with_state(state)
}

async fn create_namespace(
    State(state): State<AppState>,
    Json(ns): Json<CreateNamespace>,
) -> AppResult<Json<Namespace>> {
    let result = state.create_namespace(ns).await?;
    Ok(Json(result))
}

async fn query_namespaces(
    State(state): State<AppState>,
    Query(query): Query<QueryNamespaces>,
) -> AppResult<Json<PagedNamespaces>> {
    let result = state.query_namespaces(query).await?;
    Ok(Json(result))
}

async fn get_namespace(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Namespace>> {
    let result = state.get_namespace(&id).await?;
    Ok(Json(result))
}

async fn update_namespace(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(update): Json<UpdateNamespace>,
) -> AppResult<Json<Namespace>> {
    let result = state.update_namespace(&id, update).await?;
    Ok(Json(result))
}

async fn delete_namespace(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    state.delete_namespace(&id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}
