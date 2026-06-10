use crate::service::claim_svc;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use sqlx::MySqlPool;

pub async fn get_user_claims(
    State(pool): State<MySqlPool>,
    Path(user_id): Path<u64>,
) -> Result<impl IntoResponse, crate::error::AppError> {
    let claims = claim_svc::fetch_user_claims(&pool, user_id).await?;
    Ok(Json(claims))
}
