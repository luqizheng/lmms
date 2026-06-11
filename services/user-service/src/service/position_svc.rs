use crate::db::DbPool;
use crate::error::AppError;
use crate::models::position::{PositionDTO, PositionRow};

pub async fn list_positions(
    pool: &DbPool,
    org_unit_id: Option<u64>,
) -> Result<Vec<PositionDTO>, AppError> {
    let DbPool::Mysql(p) = pool;
    let rows: Vec<PositionRow> = if let Some(oid) = org_unit_id {
        sqlx::query_as(
            "SELECT id, code, name, org_unit_id, level, status, created_at, updated_at \
             FROM positions WHERE org_unit_id = ? ORDER BY id",
        )
        .bind(oid)
        .fetch_all(p)
        .await?
    } else {
        sqlx::query_as(
            "SELECT id, code, name, org_unit_id, level, status, created_at, updated_at \
             FROM positions ORDER BY id",
        )
        .fetch_all(p)
        .await?
    };
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let org_code: (String,) = sqlx::query_as("SELECT code FROM org_units WHERE id = ?")
            .bind(r.org_unit_id)
            .fetch_one(p)
            .await?;
        let role_codes = fetch_position_role_codes(pool, r.id).await?;
        out.push(r.into_dto(org_code.0, role_codes));
    }
    Ok(out)
}

pub async fn create_position(
    pool: &DbPool,
    code: &str,
    name: &str,
    org_unit_id: u64,
    level: i8,
) -> Result<PositionDTO, AppError> {
    let DbPool::Mysql(p) = pool;
    let res = sqlx::query(
        "INSERT INTO positions (code, name, org_unit_id, level) VALUES (?, ?, ?, ?)",
    )
    .bind(code)
    .bind(name)
    .bind(org_unit_id)
    .bind(level)
    .execute(p)
    .await?;
    get_position(pool, res.last_insert_id()).await
}

pub async fn get_position(pool: &DbPool, id: u64) -> Result<PositionDTO, AppError> {
    let DbPool::Mysql(p) = pool;
    let row: PositionRow = sqlx::query_as(
        "SELECT id, code, name, org_unit_id, level, status, created_at, updated_at \
         FROM positions WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(p)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("position id={id}")))?;
    let org: (String,) = sqlx::query_as("SELECT code FROM org_units WHERE id = ?")
        .bind(row.org_unit_id)
        .fetch_one(p)
        .await?;
    let role_codes = fetch_position_role_codes(pool, id).await?;
    Ok(row.into_dto(org.0, role_codes))
}

pub async fn update_position(
    pool: &DbPool,
    id: u64,
    name: &str,
    org_unit_id: u64,
    level: i8,
    status: i8,
) -> Result<PositionDTO, AppError> {
    let DbPool::Mysql(p) = pool;
    sqlx::query(
        "UPDATE positions SET name = ?, org_unit_id = ?, level = ?, status = ?, updated_at = NOW() \
         WHERE id = ?",
    )
    .bind(name)
    .bind(org_unit_id)
    .bind(level)
    .bind(status)
    .bind(id)
    .execute(p)
    .await?;
    get_position(pool, id).await
}

pub async fn delete_position(pool: &DbPool, id: u64) -> Result<(), AppError> {
    let DbPool::Mysql(p) = pool;
    let used: Option<(u64,)> = sqlx::query_as(
        "SELECT user_id FROM user_positions WHERE position_id = ? LIMIT 1",
    )
    .bind(id)
    .fetch_optional(p)
    .await?;
    if used.is_some() {
        return Err(AppError::Validation("position is assigned to users".into()));
    }
    sqlx::query("DELETE FROM positions WHERE id = ?")
        .bind(id)
        .execute(p)
        .await?;
    Ok(())
}

pub async fn assign_position_roles(
    pool: &DbPool,
    position_id: u64,
    role_ids: &[u64],
) -> Result<(), AppError> {
    let DbPool::Mysql(p) = pool;
    let mut tx = p.begin().await?;
    sqlx::query("DELETE FROM position_roles WHERE position_id = ?")
        .bind(position_id)
        .execute(&mut *tx)
        .await?;
    for rid in role_ids {
        sqlx::query("INSERT IGNORE INTO position_roles (position_id, role_id) VALUES (?, ?)")
            .bind(position_id)
            .bind(rid)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn fetch_position_role_codes(
    pool: &DbPool,
    position_id: u64,
) -> Result<Vec<String>, AppError> {
    let DbPool::Mysql(p) = pool;
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT r.code FROM roles r INNER JOIN position_roles pr ON pr.role_id = r.id \
         WHERE pr.position_id = ?",
    )
    .bind(position_id)
    .fetch_all(p)
    .await?;
    Ok(rows.into_iter().map(|(c,)| c).collect())
}
