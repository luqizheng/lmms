use crate::db::DbPool;
use crate::error::AppError;
use crate::models::user::{UserDTO, UserRow};

pub fn build_user_dto(row: UserRow, roles: Vec<String>) -> UserDTO {
    row.into_dto(roles)
}

pub async fn fetch_user_with_roles(
    pool: &DbPool,
    user_id: u64,
) -> Result<UserDTO, AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let row = sqlx::query_as::<_, UserRow>(
                "SELECT id, username, email, phone, nickname, status, user_code, \
                        profile_json, created_at, updated_at \
                 FROM users WHERE id = ? AND status = 1",
            )
            .bind(user_id)
            .fetch_optional(p)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("user id={user_id}")))?;

            let roles = fetch_user_role_codes(pool, user_id).await?;
            Ok(build_user_dto(row, roles))
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}

pub async fn fetch_user_by_code(
    pool: &DbPool,
    user_code: &str,
) -> Result<UserDTO, AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let row = sqlx::query_as::<_, UserRow>(
                "SELECT id, username, email, phone, nickname, status, user_code, \
                        profile_json, created_at, updated_at \
                 FROM users WHERE user_code = ? AND status = 1",
            )
            .bind(user_code)
            .fetch_optional(p)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("user_code={user_code}")))?;

            let roles = fetch_user_role_codes(pool, row.id).await?;
            Ok(build_user_dto(row, roles))
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}

pub async fn fetch_user_role_codes(
    pool: &DbPool,
    user_id: u64,
) -> Result<Vec<String>, AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT r.code FROM roles r \
                 INNER JOIN user_roles ur ON ur.role_id = r.id \
                 WHERE ur.user_id = ?",
            )
            .bind(user_id)
            .fetch_all(p)
            .await?;
            Ok(rows.into_iter().map(|(c,)| c).collect())
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}

pub async fn list_users(
    pool: &DbPool,
    page: u32,
    size: u32,
) -> Result<Vec<UserDTO>, AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let size = size.clamp(1, 100);
            let offset = (page.saturating_sub(1)) * size;
            let rows = sqlx::query_as::<_, UserRow>(
                "SELECT id, username, email, phone, nickname, status, user_code, \
                        profile_json, created_at, updated_at \
                 FROM users WHERE status = 1 ORDER BY id LIMIT ? OFFSET ?",
            )
            .bind(size)
            .bind(offset)
            .fetch_all(p)
            .await?;

            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let roles = fetch_user_role_codes(pool, row.id).await?;
                out.push(build_user_dto(row, roles));
            }
            Ok(out)
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}

pub async fn update_user_extends(
    pool: &DbPool,
    user_id: u64,
    extends: &serde_json::Value,
) -> Result<(), AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let exists: Option<(u64,)> = sqlx::query_as("SELECT id FROM users WHERE id = ?")
                .bind(user_id)
                .fetch_optional(p)
                .await?;
            if exists.is_none() {
                return Err(AppError::NotFound(format!("user id={user_id}")));
            }

            let s = serde_json::to_string(extends)
                .map_err(|e| AppError::Internal(e.into()))?;
            sqlx::query("UPDATE users SET extends_json = ?, updated_at = NOW() WHERE id = ?")
                .bind(s)
                .bind(user_id)
                .execute(p)
                .await?;
            Ok(())
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}

pub async fn assign_user_roles(
    pool: &DbPool,
    user_id: u64,
    role_ids: &[u64],
) -> Result<(), AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let mut tx = p.begin().await?;
            for role_id in role_ids {
                sqlx::query("INSERT IGNORE INTO user_roles (user_id, role_id) VALUES (?, ?)")
                    .bind(user_id)
                    .bind(role_id)
                    .execute(&mut *tx)
                    .await?;
            }
            tx.commit().await?;
            Ok(())
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}

pub async fn remove_user_roles(
    pool: &DbPool,
    user_id: u64,
    role_ids: &[u64],
) -> Result<(), AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let placeholders = vec!["?"; role_ids.len()].join(",");
            let sql = format!(
                "DELETE FROM user_roles WHERE user_id = ? AND role_id IN ({placeholders})"
            );
            let mut q = sqlx::query(&sql).bind(user_id);
            for r in role_ids {
                q = q.bind(r);
            }
            q.execute(p).await?;
            Ok(())
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}
