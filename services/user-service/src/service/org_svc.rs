use crate::db::DbPool;
use crate::error::AppError;
use crate::models::org_unit::{OrgUnitDTO, OrgUnitRow};

pub async fn list_org_units(
    pool: &DbPool,
    parent_id: Option<u64>,
) -> Result<Vec<OrgUnitDTO>, AppError> {
    let DbPool::Mysql(p) = pool;
    let rows: Vec<OrgUnitRow> = if let Some(pid) = parent_id {
        sqlx::query_as(
            "SELECT id, code, name, parent_id, sort, status, created_at, updated_at \
             FROM org_units WHERE parent_id <=> ? ORDER BY sort, id",
        )
        .bind(pid)
        .fetch_all(p)
        .await?
    } else {
        sqlx::query_as(
            "SELECT id, code, name, parent_id, sort, status, created_at, updated_at \
             FROM org_units ORDER BY sort, id",
        )
        .fetch_all(p)
        .await?
    };
    Ok(rows.into_iter().map(OrgUnitRow::into_dto).collect())
}

pub async fn create_org_unit(
    pool: &DbPool,
    code: &str,
    name: &str,
    parent_id: Option<u64>,
    sort: i32,
) -> Result<OrgUnitDTO, AppError> {
    let DbPool::Mysql(p) = pool;
    let res =
        sqlx::query("INSERT INTO org_units (code, name, parent_id, sort) VALUES (?, ?, ?, ?)")
            .bind(code)
            .bind(name)
            .bind(parent_id)
            .bind(sort)
            .execute(p)
            .await?;
    let id = res.last_insert_id();
    get_org_unit(pool, id).await
}

pub async fn get_org_unit(pool: &DbPool, id: u64) -> Result<OrgUnitDTO, AppError> {
    let DbPool::Mysql(p) = pool;
    let row: OrgUnitRow = sqlx::query_as(
        "SELECT id, code, name, parent_id, sort, status, created_at, updated_at \
         FROM org_units WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(p)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("org_unit id={id}")))?;
    Ok(row.into_dto())
}

pub async fn update_org_unit(
    pool: &DbPool,
    id: u64,
    name: &str,
    parent_id: Option<u64>,
    sort: i32,
    status: i8,
) -> Result<OrgUnitDTO, AppError> {
    let DbPool::Mysql(p) = pool;
    sqlx::query(
        "UPDATE org_units SET name = ?, parent_id = ?, sort = ?, status = ?, updated_at = NOW() \
         WHERE id = ?",
    )
    .bind(name)
    .bind(parent_id)
    .bind(sort)
    .bind(status)
    .bind(id)
    .execute(p)
    .await?;
    get_org_unit(pool, id).await
}

pub async fn delete_org_unit(pool: &DbPool, id: u64) -> Result<(), AppError> {
    let DbPool::Mysql(p) = pool;
    let child: Option<(u64,)> =
        sqlx::query_as("SELECT id FROM org_units WHERE parent_id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(p)
            .await?;
    if child.is_some() {
        return Err(AppError::Validation("org_unit has children".into()));
    }
    let pos: Option<(u64,)> =
        sqlx::query_as("SELECT id FROM positions WHERE org_unit_id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(p)
            .await?;
    if pos.is_some() {
        return Err(AppError::Validation("org_unit has positions".into()));
    }
    sqlx::query("DELETE FROM org_units WHERE id = ?")
        .bind(id)
        .execute(p)
        .await?;
    Ok(())
}

pub async fn get_org_tree(pool: &DbPool, root_id: u64) -> Result<Vec<OrgUnitDTO>, AppError> {
    let DbPool::Mysql(p) = pool;
    let rows: Vec<OrgUnitRow> = sqlx::query_as(
        "WITH RECURSIVE tree AS ( \
            SELECT id, code, name, parent_id, sort, status, created_at, updated_at \
            FROM org_units WHERE id = ? \
            UNION ALL \
            SELECT o.id, o.code, o.name, o.parent_id, o.sort, o.status, o.created_at, o.updated_at \
            FROM org_units o INNER JOIN tree t ON o.parent_id = t.id \
         ) SELECT * FROM tree ORDER BY sort, id",
    )
    .bind(root_id)
    .fetch_all(p)
    .await?;
    Ok(rows.into_iter().map(OrgUnitRow::into_dto).collect())
}
