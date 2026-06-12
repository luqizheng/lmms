use crate::db::DbPool;
use crate::dto::{ChangePasswordReq, CreateUserReq, UpdateUserReq};
use crate::error::AppError;
use crate::models::login_log::LoginLogRow;
use crate::models::user::{UserDTO, UserRow};
use crate::service::auth_svc;
use crate::service::user_position_svc;

pub fn build_user_dto(row: UserRow, roles: Vec<String>) -> UserDTO {
    row.into_dto(roles)
}

pub async fn fetch_user_with_roles(pool: &DbPool, user_id: u64) -> Result<UserDTO, AppError> {
    let DbPool::Mysql(p) = pool;
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, email, phone, nickname, status, user_code, \
                profile_json, created_at, updated_at \
         FROM users WHERE id = ? AND status = 1",
    )
    .bind(user_id)
    .fetch_optional(p)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("user id={user_id}")))?;

    let direct = fetch_user_role_codes(pool, user_id).await?;
    let pos_codes = fetch_user_position_role_codes(pool, user_id).await?;
    let mut seen = std::collections::HashSet::new();
    let roles: Vec<String> = direct
        .into_iter()
        .chain(pos_codes)
        .filter(|c| seen.insert(c.clone()))
        .collect();

    let positions = user_position_svc::list_user_positions(pool, user_id).await?;
    let mut dto = build_user_dto(row, roles);
    dto.positions = positions;
    Ok(dto)
}

pub async fn fetch_user_by_code(pool: &DbPool, user_code: &str) -> Result<UserDTO, AppError> {
    let DbPool::Mysql(p) = pool;
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, email, phone, nickname, status, user_code, \
                profile_json, created_at, updated_at \
         FROM users WHERE user_code = ? AND status = 1",
    )
    .bind(user_code)
    .fetch_optional(p)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("user_code={user_code}")))?;

    let direct = fetch_user_role_codes(pool, row.id).await?;
    let pos_codes = fetch_user_position_role_codes(pool, row.id).await?;
    let mut seen = std::collections::HashSet::new();
    let roles: Vec<String> = direct
        .into_iter()
        .chain(pos_codes)
        .filter(|c| seen.insert(c.clone()))
        .collect();

    let positions = user_position_svc::list_user_positions(pool, row.id).await?;
    let mut dto = build_user_dto(row, roles);
    dto.positions = positions;
    Ok(dto)
}

pub async fn fetch_user_role_codes(pool: &DbPool, user_id: u64) -> Result<Vec<String>, AppError> {
    let DbPool::Mysql(p) = pool;
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

async fn fetch_user_position_role_codes(
    pool: &DbPool,
    user_id: u64,
) -> Result<Vec<String>, AppError> {
    let DbPool::Mysql(p) = pool;
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT r.code FROM roles r \
         INNER JOIN position_roles pr ON pr.role_id = r.id \
         INNER JOIN user_positions up ON up.position_id = pr.position_id \
         WHERE up.user_id = ?",
    )
    .bind(user_id)
    .fetch_all(p)
    .await?;
    Ok(rows.into_iter().map(|(c,)| c).collect())
}

pub async fn list_users(pool: &DbPool, page: u32, size: u32) -> Result<Vec<UserDTO>, AppError> {
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

            let s = serde_json::to_string(extends).map_err(|e| AppError::Internal(e.into()))?;
            sqlx::query("UPDATE users SET extends_json = ?, updated_at = NOW() WHERE id = ?")
                .bind(s)
                .bind(user_id)
                .execute(p)
                .await?;
            Ok(())
        }
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
            let sql =
                format!("DELETE FROM user_roles WHERE user_id = ? AND role_id IN ({placeholders})");
            let mut q = sqlx::query(&sql).bind(user_id);
            for r in role_ids {
                q = q.bind(r);
            }
            q.execute(p).await?;
            Ok(())
        }
    }
}

const MIN_PASSWORD_LEN: usize = 6;

fn is_dup(err: &dyn sqlx::error::DatabaseError) -> bool {
    err.code().as_deref() == Some("23000")
}

pub async fn create_user(pool: &DbPool, req: &CreateUserReq) -> Result<u64, AppError> {
    if req.username.trim().is_empty() {
        return Err(AppError::Validation("username is required".into()));
    }
    if req.password.len() < MIN_PASSWORD_LEN {
        return Err(AppError::Validation(format!(
            "password must be at least {MIN_PASSWORD_LEN} chars"
        )));
    }
    let hash = auth_svc::hash_password(&req.password)?;
    let user_code = format!("USR-{}", uuid::Uuid::new_v4().simple());
    if let DbPool::Mysql(p) = pool {
        let res = sqlx::query(
            "INSERT INTO users (username, user_code, password_hash, email, phone, nickname, status) \
             VALUES (?, ?, ?, ?, ?, ?, 1)",
        )
        .bind(&req.username)
        .bind(&user_code)
        .bind(&hash)
        .bind(req.email.as_deref())
        .bind(req.phone.as_deref())
        .bind(req.nickname.as_deref())
        .execute(p)
        .await;
        match res {
            Ok(r) => Ok(r.last_insert_id()),
            Err(sqlx::Error::Database(db_err)) if is_dup(&*db_err) => {
                Err(AppError::Conflict("username or contact already exists".into()))
            }
            Err(e) => Err(AppError::Db(e)),
        }
    } else {
        Err(AppError::UnsupportedDb(pool.db_type()))
    }
}

pub async fn soft_delete_user(pool: &DbPool, id: u64) -> Result<(), AppError> {
    if let DbPool::Mysql(p) = pool {
        let res = sqlx::query("UPDATE users SET status = 0, updated_at = NOW() WHERE id = ?")
            .bind(id)
            .execute(p)
            .await?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("user id={id}")));
        }
        Ok(())
    } else {
        Err(AppError::UnsupportedDb(pool.db_type()))
    }
}

pub async fn set_user_status(pool: &DbPool, id: u64, status: i8) -> Result<(), AppError> {
    if !matches!(status, 0 | 1) {
        return Err(AppError::Validation("status must be 0 or 1".into()));
    }
    if let DbPool::Mysql(p) = pool {
        let res = sqlx::query("UPDATE users SET status = ?, updated_at = NOW() WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(p)
            .await?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("user id={id}")));
        }
        Ok(())
    } else {
        Err(AppError::UnsupportedDb(pool.db_type()))
    }
}

pub async fn update_user_basic(
    pool: &DbPool,
    id: u64,
    req: &UpdateUserReq,
) -> Result<(), AppError> {
    let mut sets: Vec<&str> = Vec::new();
    if req.username.is_some() {
        sets.push("username = ?");
    }
    if req.email.is_some() {
        sets.push("email = ?");
    }
    if req.phone.is_some() {
        sets.push("phone = ?");
    }
    if req.nickname.is_some() {
        sets.push("nickname = ?");
    }
    if sets.is_empty() {
        return Err(AppError::Validation("no fields to update".into()));
    }
    let sql = format!(
        "UPDATE users SET {}, updated_at = NOW() WHERE id = ?",
        sets.join(", ")
    );
    if let DbPool::Mysql(p) = pool {
        let mut q = sqlx::query(&sql);
        if let Some(v) = &req.username {
            q = q.bind(v);
        }
        if let Some(v) = &req.email {
            q = q.bind(v);
        }
        if let Some(v) = &req.phone {
            q = q.bind(v);
        }
        if let Some(v) = &req.nickname {
            q = q.bind(v);
        }
        let res = q.bind(id).execute(p).await;
        match res {
            Ok(r) if r.rows_affected() == 0 => {
                Err(AppError::NotFound(format!("user id={id}")))
            }
            Ok(_) => Ok(()),
            Err(sqlx::Error::Database(db_err)) if is_dup(&*db_err) => {
                Err(AppError::Conflict("username or contact already exists".into()))
            }
            Err(e) => Err(AppError::Db(e)),
        }
    } else {
        Err(AppError::UnsupportedDb(pool.db_type()))
    }
}

pub async fn reset_password(pool: &DbPool, id: u64, new_password: &str) -> Result<(), AppError> {
    if new_password.len() < MIN_PASSWORD_LEN {
        return Err(AppError::Validation(format!(
            "password must be at least {MIN_PASSWORD_LEN} chars"
        )));
    }
    let hash = auth_svc::hash_password(new_password)?;
    if let DbPool::Mysql(p) = pool {
        let res = sqlx::query("UPDATE users SET password_hash = ?, updated_at = NOW() WHERE id = ?")
            .bind(&hash)
            .bind(id)
            .execute(p)
            .await?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("user id={id}")));
        }
        Ok(())
    } else {
        Err(AppError::UnsupportedDb(pool.db_type()))
    }
}

pub async fn change_password(
    pool: &DbPool,
    id: u64,
    req: &ChangePasswordReq,
) -> Result<(), AppError> {
    if req.new_password.len() < MIN_PASSWORD_LEN {
        return Err(AppError::Validation(format!(
            "password must be at least {MIN_PASSWORD_LEN} chars"
        )));
    }
    if req.old_password == req.new_password {
        return Err(AppError::Validation(
            "new password must differ from old".into(),
        ));
    }
    if let DbPool::Mysql(p) = pool {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT password_hash FROM users WHERE id = ?")
                .bind(id)
                .fetch_optional(p)
                .await?;
        let hash = row
            .ok_or_else(|| AppError::NotFound(format!("user id={id}")))?
            .0;
        if !auth_svc::verify_password(&req.old_password, &hash) {
            return Err(AppError::Unauthorized);
        }
        let new_hash = auth_svc::hash_password(&req.new_password)?;
        sqlx::query("UPDATE users SET password_hash = ?, updated_at = NOW() WHERE id = ?")
            .bind(&new_hash)
            .bind(id)
            .execute(p)
            .await?;
        Ok(())
    } else {
        Err(AppError::UnsupportedDb(pool.db_type()))
    }
}

pub async fn list_login_logs(
    pool: &DbPool,
    id: u64,
    page: u32,
    size: u32,
) -> Result<Vec<LoginLogRow>, AppError> {
    if let DbPool::Mysql(p) = pool {
        let size = size.clamp(1, 100);
        let offset = page.saturating_sub(1) * size;
        let rows = sqlx::query_as::<_, LoginLogRow>(
            "SELECT id, user_id, username, ip, user_agent, success, fail_reason, login_at \
             FROM user_login_log WHERE user_id = ? \
             ORDER BY id DESC LIMIT ? OFFSET ?",
        )
        .bind(id)
        .bind(size)
        .bind(offset)
        .fetch_all(p)
        .await?;
        Ok(rows)
    } else {
        Err(AppError::UnsupportedDb(pool.db_type()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_password_len_constant() {
        assert_eq!(MIN_PASSWORD_LEN, 6);
    }
}
