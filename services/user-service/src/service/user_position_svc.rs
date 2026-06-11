use crate::db::DbPool;
use crate::error::AppError;
use crate::models::user_position::UserPositionDTO;

#[derive(Debug, Clone)]
pub struct PositionAssignItem {
    pub position_id: u64,
    pub is_primary: bool,
}

pub async fn list_user_positions(
    pool: &DbPool,
    user_id: u64,
) -> Result<Vec<UserPositionDTO>, AppError> {
    let DbPool::Mysql(p) = pool;
    let rows: Vec<(u64, String, String, u64, String, i8, String)> = sqlx::query_as(
        "SELECT up.position_id, p.code, p.name, ou.id, ou.name, up.is_primary, up.assigned_at \
         FROM user_positions up \
         INNER JOIN positions p ON p.id = up.position_id \
         INNER JOIN org_units ou ON ou.id = p.org_unit_id \
         WHERE up.user_id = ? ORDER BY up.is_primary DESC, up.assigned_at",
    )
    .bind(user_id)
    .fetch_all(p)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(pid, pcode, pname, ouid, ourname, prim, assigned_at)| UserPositionDTO {
                position_id: pid,
                position_code: pcode,
                position_name: pname,
                org_unit_id: ouid,
                org_unit_name: ourname,
                is_primary: prim != 0,
                assigned_at,
            },
        )
        .collect())
}

pub async fn replace_user_positions(
    pool: &DbPool,
    user_id: u64,
    items: &[PositionAssignItem],
) -> Result<Vec<UserPositionDTO>, AppError> {
    let DbPool::Mysql(p) = pool;
    let mut tx = p.begin().await?;
    sqlx::query("DELETE FROM user_positions WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let primary_count = items.iter().filter(|i| i.is_primary).count();
    if primary_count > 1 {
        return Err(AppError::Validation("only one primary position allowed".into()));
    }
    for it in items {
        sqlx::query(
            "INSERT INTO user_positions (user_id, position_id, is_primary) VALUES (?, ?, ?)",
        )
        .bind(user_id)
        .bind(it.position_id)
        .bind(if it.is_primary { 1 } else { 0 })
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    list_user_positions(pool, user_id).await
}
