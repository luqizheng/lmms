use crate::error::AppError;
use crate::service::claim_svc;
use axum::{extract::State, response::IntoResponse, Json};
use sqlx::MySqlPool;

pub async fn list_permissions(
    State(pool): State<MySqlPool>,
) -> Result<impl IntoResponse, AppError> {
    let items = claim_svc::list_permissions(&pool).await?;
    Ok(Json(items))
}
