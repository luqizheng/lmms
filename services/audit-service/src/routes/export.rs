use axum::{extract::State, extract::Query, http::header::CONTENT_TYPE, response::{IntoResponse, Response}, http::StatusCode};
use crate::error::AppResult;
use crate::models::operation::{QueryOperationLogs, OperationLog};
use crate::routes::AppState;

pub fn routes(state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/export/operation/csv", axum::routing::get(export_operation_csv))
        .with_state(state)
}

async fn export_operation_csv(
    State(state): State<AppState>,
    Query(query): Query<QueryOperationLogs>,
) -> AppResult<Response> {
    let page = query.page.unwrap_or(1);
    let size = query.size.unwrap_or(10000);
    let query = QueryOperationLogs { page: Some(page), size: Some(size), ..query };

    let result = state.query_operation_logs(query).await?;
    
    let mut csv = String::from("id,user_id,action,resource,resource_id,before_value,after_value,request_id,ip_address,created_at\n");
    for log in result.items {
        csv.push_str(&format_operation_csv_row(&log));
        csv.push('\n');
    }

    let body = csv.into_response();
    let mut res = body.into_response();
    res.headers_mut().insert(CONTENT_TYPE, "text/csv; charset=utf-8".parse().unwrap());
    Ok(res)
}

fn format_operation_csv_row(log: &OperationLog) -> String {
    format!(
        "{},{},{},{},{},{},{},{},{},{}",
        log.id,
        escape_csv(&log.user_id),
        escape_csv(&log.action),
        escape_csv(&log.resource),
        escape_csv(&log.resource_id.as_deref().unwrap_or("")),
        escape_csv(&log.before_value.as_deref().unwrap_or("")),
        escape_csv(&log.after_value.as_deref().unwrap_or("")),
        escape_csv(&log.request_id),
        escape_csv(&log.ip_address.as_deref().unwrap_or("")),
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
