use crate::error::AppError;
use crate::service::claim_svc;
use axum::{extract::State, response::IntoResponse, Json};
use user_service::db::DbPool;

pub async fn list_permissions(
    State(pool): State<DbPool>,
) -> Result<impl IntoResponse, AppError> {
    let items = claim_svc::list_permissions(&pool).await?;
    Ok(Json(items))
}
