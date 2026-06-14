use axum::{extract::State, extract::Query, http::header, response::IntoResponse, Router};
use crate::error::AppResult;
use crate::models::operation::QueryOperationLogs;
use crate::routes::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/export/operation/csv", axum::routing::get(export_operation_csv))
        .route("/export/operation/json", axum::routing::get(export_operation_json))
        .with_state(state)
}

async fn export_operation_csv(
    State(state): State<AppState>,
    Query(query): Query<QueryOperationLogs>,
) -> AppResult<impl IntoResponse> {
    let page = query.page.unwrap_or(1);
    let size = query.size.unwrap_or(10000);
    let query = QueryOperationLogs { page: Some(page), size: Some(size), ..query };

    let result = state.audit.query_operation_logs(query).await?;
    
    let mut csv = String::from("id,user_id,action,resource,resource_id,before_value,after_value,request_id,ip_address,created_at\n");
    for log in result.items {
        csv.push_str(&format_operation_csv_row(&log));
        csv.push('\n');
    }

    Ok((
        [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
        csv,
    ))
}

async fn export_operation_json(
    State(state): State<AppState>,
    Query(query): Query<QueryOperationLogs>,
) -> AppResult<impl IntoResponse> {
    let page = query.page.unwrap_or(1);
    let size = query.size.unwrap_or(10000);
    let query = QueryOperationLogs { page: Some(page), size: Some(size), ..query };

    let result = state.audit.query_operation_logs(query).await?;
    let json = serde_json::to_string_pretty(&result.items)
        .map_err(|e| crate::error::AppError::internal("EXPORT_JSON", e.to_string()))?;

    Ok((
        [(header::CONTENT_TYPE, "application/json")],
        json,
    ))
}

fn format_operation_csv_row(log: &crate::models::OperationLog) -> String {
    format!(
        "{},{},{},{},{},{},{},{},{},{}",
        log.id,
        escape_csv(&log.user_id),
        escape_csv(&log.action),
        escape_csv(&log.resource),
        escape_csv(log.resource_id.as_deref().unwrap_or("")),
        escape_csv(log.before_value.as_deref().unwrap_or("")),
        escape_csv(log.after_value.as_deref().unwrap_or("")),
        escape_csv(&log.request_id),
        escape_csv(log.ip_address.as_deref().unwrap_or("")),
        log.created_at.to_rfc3339()
    )
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
