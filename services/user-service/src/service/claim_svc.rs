use crate::db::DbPool;
use crate::error::AppError;
use crate::models::permission::PermissionRow;

#[derive(Debug, serde::Serialize)]
pub struct UserClaims {
    pub user_id: u64,
    pub username: String,
    pub user_code: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub status: i8,
}

pub async fn fetch_user_claims(pool: &DbPool, user_id: u64) -> Result<UserClaims, AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let row: (u64, String, String, i8) =
                sqlx::query_as("SELECT id, username, user_code, status FROM users WHERE id = ?")
                    .bind(user_id)
                    .fetch_optional(p)
                    .await?
                    .ok_or_else(|| AppError::NotFound(format!("user id={user_id}")))?;

            let roles: Vec<(String,)> = sqlx::query_as(
                "SELECT r.code FROM roles r \
                 INNER JOIN user_roles ur ON ur.role_id = r.id \
                 WHERE ur.user_id = ?",
            )
            .bind(user_id)
            .fetch_all(p)
            .await?;

            let perms: Vec<(String,)> = sqlx::query_as(
                "SELECT DISTINCT p.code FROM permissions p \
                 INNER JOIN role_permissions rp ON rp.permission_id = p.id \
                 INNER JOIN user_roles ur ON ur.role_id = rp.role_id \
                 WHERE ur.user_id = ?",
            )
            .bind(user_id)
            .fetch_all(p)
            .await?;

            Ok(UserClaims {
                user_id: row.0,
                username: row.1,
                user_code: row.2,
                status: row.3,
                roles: roles.into_iter().map(|(c,)| c).collect(),
                permissions: perms.into_iter().map(|(c,)| c).collect(),
            })
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}

pub async fn list_permissions(pool: &DbPool) -> Result<Vec<PermissionRow>, AppError> {
    match pool {
        DbPool::Mysql(p) => {
            let rows = sqlx::query_as::<_, PermissionRow>(
                "SELECT id, code, name, resource, action FROM permissions ORDER BY id",
            )
            .fetch_all(p)
            .await?;
            Ok(rows)
        }
        _ => Err(AppError::UnsupportedDb(pool.db_type())),
    }
}
