use crate::{
    error::IamError,
    models::User,
    state::AppState,
    dto::{CreateUserRequest, UpdateUserRequest, UserQuery, UserResponse, UserListResponse},
};
use common_crypto::password::hash_password;
use chrono::Utc;
use nanoid::nanoid;
use sqlx::{FromRow, mysql::MySqlQueryResult};

pub struct UserService {
    state: AppState,
}

#[derive(FromRow)]
struct UserRow {
    id: String,
    username: String,
    email: Option<String>,
    phone: Option<String>,
    password_hash: String,
    salt: String,
    nickname: String,
    avatar: Option<String>,
    status: String,
    email_verified: bool,
    phone_verified: bool,
    last_login_at: Option<chrono::DateTime<Utc>>,
    last_login_ip: Option<String>,
    password_expired_at: Option<chrono::DateTime<Utc>>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
    deleted_at: Option<chrono::DateTime<Utc>>,
}

impl UserService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(&self, req: CreateUserRequest) -> Result<UserResponse, IamError> {
        let user_id = nanoid!();
        let (password_hash, salt) = hash_password(&req.password)?;

        let user: UserRow = sqlx::query_as(
            r#"INSERT INTO users (id, username, password_hash, salt, nickname, email, phone, avatar, status, created_at, updated_at) 
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', NOW(), NOW()) 
               RETURNING id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at"#,
        )
        .bind(&user_id)
        .bind(&req.username)
        .bind(&password_hash)
        .bind(&salt)
        .bind(&req.nickname)
        .bind(&req.email)
        .bind(&req.phone)
        .bind(&req.avatar)
        .fetch_one(&self.state.db)
        .await?;

        for role_id in req.role_ids {
            let _ = sqlx::query(
                "INSERT INTO user_roles (user_id, role_id, created_at) VALUES (?, ?, NOW())",
            )
            .bind(&user_id)
            .bind(&role_id)
            .execute(&self.state.db)
            .await;
        }

        Ok(self.to_response(&user))
    }

    pub async fn get(&self, user_id: &str) -> Result<UserResponse, IamError> {
        let user: Option<UserRow> = sqlx::query_as(
            "SELECT id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM users WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_optional(&self.state.db)
        .await?;

        match user {
            Some(u) => Ok(self.to_response(&u)),
            None => Err(IamError::UserNotFound),
        }
    }

    pub async fn list(&self, query: UserQuery) -> Result<UserListResponse, IamError> {
        let offset = ((query.page - 1) * query.size) as i64;
        let limit = query.size as i64;

        let users: Vec<UserRow> = if let Some(keyword) = &query.keyword {
            let pattern = format!("%{}%", keyword);
            if let Some(status) = &query.status {
                sqlx::query_as(
                    "SELECT id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM users WHERE deleted_at IS NULL AND (username LIKE ? OR nickname LIKE ? OR email LIKE ?) AND status = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
                )
                .bind(&pattern)
                .bind(&pattern)
                .bind(&pattern)
                .bind(status)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.state.db)
                .await?
            } else {
                sqlx::query_as(
                    "SELECT id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM users WHERE deleted_at IS NULL AND (username LIKE ? OR nickname LIKE ? OR email LIKE ?) ORDER BY created_at DESC LIMIT ? OFFSET ?",
                )
                .bind(&pattern)
                .bind(&pattern)
                .bind(&pattern)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.state.db)
                .await?
            }
        } else if let Some(status) = &query.status {
            sqlx::query_as(
                "SELECT id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM users WHERE deleted_at IS NULL AND status = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.state.db)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM users WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.state.db)
            .await?
        };

        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE deleted_at IS NULL")
            .fetch_one(&self.state.db)
            .await?;

        Ok(UserListResponse {
            items: users.into_iter().map(|u| self.to_response(&u)).collect(),
            total: total.0 as u64,
            page: query.page,
            size: query.size,
        })
    }

    pub async fn update(&self, user_id: &str, req: UpdateUserRequest) -> Result<UserResponse, IamError> {
        let user: Option<UserRow> = sqlx::query_as(
            "SELECT id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM users WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_optional(&self.state.db)
        .await?;

        let user = match user {
            Some(u) => u,
            None => return Err(IamError::UserNotFound),
        };

        let nickname = req.nickname.unwrap_or(user.nickname);
        let email = req.email.or(user.email);
        let phone = req.phone.or(user.phone);
        let avatar = req.avatar.or(user.avatar);
        let status = req.status.unwrap_or(user.status);

        let result: UserRow = sqlx::query_as(
            "UPDATE users SET nickname = ?, email = ?, phone = ?, avatar = ?, status = ?, updated_at = NOW() WHERE id = ? RETURNING id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at",
        )
        .bind(&nickname)
        .bind(&email)
        .bind(&phone)
        .bind(&avatar)
        .bind(&status)
        .bind(user_id)
        .fetch_one(&self.state.db)
        .await?;

        Ok(self.to_response(&result))
    }

    pub async fn delete(&self, user_id: &str) -> Result<(), IamError> {
        let result: MySqlQueryResult = sqlx::query(
            "UPDATE users SET deleted_at = NOW(), updated_at = NOW() WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(user_id)
        .execute(&self.state.db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(IamError::UserNotFound);
        }

        Ok(())
    }

    fn to_response(&self, user: &UserRow) -> UserResponse {
        UserResponse {
            id: user.id.clone(),
            username: user.username.clone(),
            email: user.email.clone(),
            phone: user.phone.clone(),
            nickname: user.nickname.clone(),
            avatar: user.avatar.clone(),
            status: user.status.clone(),
            email_verified: user.email_verified,
            phone_verified: user.phone_verified,
            last_login_at: user.last_login_at,
            last_login_ip: user.last_login_ip.clone(),
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}
