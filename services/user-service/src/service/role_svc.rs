use crate::db::DbPool;
use crate::error::AppError;
use crate::models::role::{RoleDTO, RoleRow};

pub async fn list_roles(pool: &DbPool) -> Result<Vec<RoleDTO>, AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let rows = sqlx::query_as::<_, RoleRow>(
                "SELECT id, code, name, description, created_at, updated_at \
                 FROM roles ORDER BY id",
            )
            .fetch_all(p)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let perms = fetch_role_permission_codes(pool, row.id).await?;
                out.push(row.into_dto(perms));
            }
            Ok(out)
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}

pub async fn get_role(pool: &DbPool, role_id: u64) -> Result<RoleDTO, AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let row = sqlx::query_as::<_, RoleRow>(
                "SELECT id, code, name, description, created_at, updated_at \
                 FROM roles WHERE id = ?",
            )
            .bind(role_id)
            .fetch_optional(p)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("role id={role_id}")))?;
            let perms = fetch_role_permission_codes(pool, role_id).await?;
            Ok(row.into_dto(perms))
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}

pub async fn create_role(
    pool: &DbPool,
    code: &str,
    name: &str,
    description: Option<&str>,
) -> Result<RoleDTO, AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let res = sqlx::query("INSERT INTO roles (code, name, description) VALUES (?, ?, ?)")
                .bind(code)
                .bind(name)
                .bind(description)
                .execute(p)
                .await;

            let id = match res {
                Ok(r) => r.last_insert_id(),
                Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("23000") => {
                    return Err(AppError::Conflict(format!("role code={code}")));
                }
                Err(e) => return Err(e.into()),
            };
            get_role(pool, id).await
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}

pub async fn update_role(
    pool: &DbPool,
    role_id: u64,
    name: &str,
    description: Option<&str>,
) -> Result<RoleDTO, AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let res = sqlx::query(
                "UPDATE roles SET name = ?, description = ?, updated_at = NOW() WHERE id = ?",
            )
            .bind(name)
            .bind(description)
            .bind(role_id)
            .execute(p)
            .await?;
            if res.rows_affected() == 0 {
                return Err(AppError::NotFound(format!("role id={role_id}")));
            }
            get_role(pool, role_id).await
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}

pub async fn delete_role(pool: &DbPool, role_id: u64) -> Result<(), AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let res = sqlx::query("DELETE FROM roles WHERE id = ?")
                .bind(role_id)
                .execute(p)
                .await?;
            if res.rows_affected() == 0 {
                return Err(AppError::NotFound(format!("role id={role_id}")));
            }
            Ok(())
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}

pub async fn fetch_role_permission_codes(
    pool: &DbPool,
    role_id: u64,
) -> Result<Vec<String>, AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT p.code FROM permissions p \
                 INNER JOIN role_permissions rp ON rp.permission_id = p.id \
                 WHERE rp.role_id = ?",
            )
            .bind(role_id)
            .fetch_all(p)
            .await?;
            Ok(rows.into_iter().map(|(c,)| c).collect())
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}

pub async fn assign_role_permissions(
    pool: &DbPool,
    role_id: u64,
    permission_ids: &[u64],
) -> Result<(), AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let mut tx = p.begin().await?;
            for pid in permission_ids {
                sqlx::query(
                    "INSERT IGNORE INTO role_permissions (role_id, permission_id) VALUES (?, ?)",
                )
                .bind(role_id)
                .bind(pid)
                .execute(&mut *tx)
                .await?;
            }
            tx.commit().await?;
            Ok(())
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}
