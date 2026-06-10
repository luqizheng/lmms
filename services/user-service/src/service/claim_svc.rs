use crate::error::AppError;
use crate::models::permission::PermissionRow;
use sqlx::MySqlPool;

#[derive(Debug, serde::Serialize)]
pub struct UserClaims {
    pub user_id: u64,
    pub username: String,
    pub user_code: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub status: i8,
}

pub async fn fetch_user_claims(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<UserClaims, AppError> {
    let row: (u64, String, String, i8) = sqlx::query_as(
        "SELECT id, username, user_code, status FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("user id={user_id}")))?;

    let roles: Vec<(String,)> = sqlx::query_as(
        "SELECT r.code FROM roles r \
         INNER JOIN user_roles ur ON ur.role_id = r.id \
         WHERE ur.user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let perms: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT p.code FROM permissions p \
         INNER JOIN role_permissions rp ON rp.permission_id = p.id \
         INNER JOIN user_roles ur ON ur.role_id = rp.role_id \
         WHERE ur.user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
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

pub async fn list_permissions(pool: &MySqlPool) -> Result<Vec<PermissionRow>, AppError> {
    let rows = sqlx::query_as::<_, PermissionRow>(
        "SELECT id, code, name, resource, action FROM permissions ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
