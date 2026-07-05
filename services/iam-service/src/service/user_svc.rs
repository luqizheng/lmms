use crate::{
    error::IamError,
    state::AppState,
    dto::{CreateUserRequest, UpdateUserRequest, UserQuery, UserResponse, UserListResponse, UserSimpleQuery, UserSimpleResponse, UserSimpleListResponse, UserClaimsResponse},
};
use common_crypto::password::hash;
use common_db::WithTenantScope;
use common_event::events::{IamUserCreated, IamUserDeleted};
use common_event::{Envelope, OutboxRepo};
use chrono::Utc;
use jwt_ctx::TenantScope;
use nanoid::nanoid;
use sqlx::{FromRow, QueryBuilder, mysql::MySqlQueryResult};

#[derive(FromRow)]
struct UserRow {
    id: String,
    tenant_id: u64,
    username: String,
    email: Option<String>,
    phone: Option<String>,
    password_hash: String,
    salt: Option<String>,
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

pub struct UserService {
    state: AppState,
}

impl UserService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(&self, req: CreateUserRequest, scope: &TenantScope) -> Result<UserResponse, IamError> {
        let tenant_id = scope.tenant_id().unwrap_or(1);
        let tid = tenant_id as u64;

        let exists: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM iam_users WHERE username = ? AND tenant_id = ? AND deleted_at IS NULL"
        )
        .bind(&req.username)
        .bind(tid)
        .fetch_optional(&self.state.db)
        .await?;

        if exists.is_some() {
            return Err(IamError::DuplicateUser);
        }

        let password_hash = hash(&req.password)
            .map_err(|e| IamError::Jwt(e.to_string()))?;

        let user_id = nanoid!();
        let salt = nanoid!(32);

        let user_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM iam_users WHERE deleted_at IS NULL"
        )
        .fetch_one(&self.state.db)
        .await?;
        let mut role_ids = req.role_ids.clone().unwrap_or_default();
        if user_count.0 == 0 {
            let role_id = nanoid!();
            sqlx::query(
                "INSERT INTO iam_roles (id, tenant_id, name, code, description, data_scope, status, created_at, updated_at) \
                 VALUES (?, ?, '租户管理员', 'tenant_admin', '首用户自动创建', 'all', 'active', NOW(), NOW())",
            )
            .bind(&role_id)
            .bind(tid)
            .execute(&self.state.db)
            .await?;
            sqlx::query(
                "INSERT INTO iam_role_permissions (role_id, permission_id, created_at) \
                 SELECT ?, id, NOW() FROM iam_permissions WHERE status = 'enabled' AND deleted_at IS NULL",
            )
            .bind(&role_id)
            .execute(&self.state.db)
            .await?;
            role_ids.push(role_id);
        }

        let mut tx = self.state.db.begin().await?;
        sqlx::query(
            "INSERT INTO iam_users (id, tenant_id, username, password_hash, salt, nickname, email, phone, avatar, status, email_verified, phone_verified, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', false, false, NOW(), NOW())"
        )
        .bind(&user_id)
        .bind(tid)
        .bind(&req.username)
        .bind(&password_hash)
        .bind(&salt)
        .bind(&req.nickname)
        .bind(&req.email)
        .bind(&req.phone)
        .bind(&req.avatar)
        .execute(&mut *tx)
        .await?;

        let mut envelope = Envelope::new(
            "iam-service",
            IamUserCreated {
                user_id: user_id.clone(),
                username: req.username.clone(),
                email: req.email.clone(),
                org_unit_id: None,
            },
        )
        .with_tenant_id(tenant_id.to_string());
        self.state.outbox_repo.clone()
            .enqueue(&mut tx, &mut envelope)
            .await
            .map_err(|e| IamError::Internal(e.to_string()))?;

        crate::service::user_roles::set_user_roles_in_tx(&mut tx, &user_id, &role_ids).await?;

        tx.commit().await?;

        let mut qb = QueryBuilder::new(
            "SELECT id, tenant_id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM iam_users",
        );
        qb.where_id(&user_id);
        qb.and_tenant(scope);
        let user: UserRow = qb.build_query_as::<User>().fetch_one(&self.state.db).await?;

        Ok(self.to_response(&user))
    }

    pub async fn get(&self, user_id: &str, scope: &TenantScope) -> Result<UserResponse, IamError> {
        let mut qb = QueryBuilder::new(
            "SELECT id, tenant_id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM iam_users",
        );
        qb.where_id(user_id);
        qb.and_tenant(scope);
        qb.push(" AND deleted_at IS NULL");
        let user: Option<UserRow> = qb.build_query_as::<UserRow>().fetch_optional(&self.state.db).await?;

        match user {
            Some(u) => Ok(self.to_response(&u)),
            None => Err(IamError::UserNotFound),
        }
    }

    pub async fn list(&self, query: UserQuery, scope: &TenantScope) -> Result<UserListResponse, IamError> {
        let offset = ((query.page - 1) * query.size) as i64;
        let limit = query.size as i64;
        let tid = scope.tenant_id().unwrap_or(1) as u64;

        let users: Vec<UserRow> = if let Some(keyword) = &query.keyword {
            let pattern = format!("%{}%", keyword);
            sqlx::query_as(
                "SELECT id, tenant_id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM iam_users WHERE tenant_id = ? AND deleted_at IS NULL AND (username LIKE ? OR nickname LIKE ? OR email LIKE ?) ORDER BY created_at DESC LIMIT ? OFFSET ?"
            )
            .bind(tid)
            .bind(&pattern)
            .bind(&pattern)
            .bind(&pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.state.db)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, tenant_id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM iam_users WHERE tenant_id = ? AND deleted_at IS NULL ORDER BY created_at DESC LIMIT ? OFFSET ?"
            )
            .bind(tid)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.state.db)
            .await?
        };

        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM iam_users WHERE tenant_id = ? AND deleted_at IS NULL")
            .bind(tid)
            .fetch_one(&self.state.db)
            .await?;

        Ok(UserListResponse {
            items: users.into_iter().map(|u| self.to_response(&u)).collect(),
            total: total.0 as u64,
            page: query.page,
            size: query.size,
        })
    }

    pub async fn update(&self, user_id: &str, req: UpdateUserRequest, scope: &TenantScope) -> Result<UserResponse, IamError> {
        let mut qb = QueryBuilder::new(
            "SELECT id, tenant_id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM iam_users",
        );
        qb.where_id(user_id);
        qb.and_tenant(scope);
        qb.push(" AND deleted_at IS NULL");
        let existing: Option<UserRow> = qb.build_query_as::<UserRow>().fetch_optional(&self.state.db).await?;

        let existing = match existing {
            Some(u) => u,
            None => return Err(IamError::UserNotFound),
        };

        let nickname = req.nickname.unwrap_or(existing.nickname);
        let email = req.email.or(existing.email);
        let phone = req.phone.or(existing.phone);
        let avatar = req.avatar.or(existing.avatar);
        let status = req.status.unwrap_or(existing.status);

        let mut qb = QueryBuilder::new("UPDATE iam_users SET nickname = ");
        qb.push_bind(&nickname);
        qb.push(", email = ");
        qb.push_bind(&email);
        qb.push(", phone = ");
        qb.push_bind(&phone);
        qb.push(", avatar = ");
        qb.push_bind(&avatar);
        qb.push(", status = ");
        qb.push_bind(&status);
        qb.push(", updated_at = NOW()");
        qb.where_id(user_id);
        qb.and_tenant(scope);
        qb.push(" AND deleted_at IS NULL");
        qb.build()
            .execute(&self.state.db)
            .await
            .map(|_| ())?;

        let mut qb = QueryBuilder::new(
            "SELECT id, tenant_id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM iam_users",
        );
        qb.where_id(user_id);
        qb.and_tenant(scope);
        qb.push(" AND deleted_at IS NULL");
        let user: UserRow = qb.build_query_as::<UserRow>().fetch_one(&self.state.db).await?;

        Ok(self.to_response(&user))
    }

    pub async fn delete(&self, user_id: &str, scope: &TenantScope) -> Result<(), IamError> {
        let tenant_id = scope.tenant_id().unwrap_or(1);

        let mut tx = self.state.db.begin().await?;
        let mut qb = QueryBuilder::new("UPDATE iam_users SET deleted_at = NOW(), updated_at = NOW()");
        qb.where_id(user_id);
        qb.and_tenant(scope);
        qb.push(" AND deleted_at IS NULL");
        let result: MySqlQueryResult = qb.build().execute(&mut *tx).await?;

        if result.rows_affected() == 0 {
            return Err(IamError::UserNotFound);
        }

        let mut envelope = Envelope::new(
            "iam-service",
            IamUserDeleted {
                user_id: user_id.to_string(),
                deleted_by: "system".to_string(),
            },
        )
        .with_tenant_id(tenant_id.to_string());
        self.state.outbox_repo.clone()
            .enqueue(&mut tx, &mut envelope)
            .await
            .map_err(|e| IamError::Internal(e.to_string()))?;
        tx.commit().await?;

        Ok(())
    }

    pub async fn list_simple(&self, query: UserSimpleQuery, scope: &TenantScope) -> Result<UserSimpleListResponse, IamError> {
        let offset = ((query.page - 1) * query.size) as i64;
        let limit = query.size as i64;
        let tid = scope.tenant_id().unwrap_or(1) as u64;

        let users: Vec<UserSimpleResponse> = if let Some(keyword) = &query.keyword {
            let pattern = format!("%{}%", keyword);
            sqlx::query_as(
                "SELECT id, nickname FROM iam_users WHERE tenant_id = ? AND deleted_at IS NULL AND status = 'active' AND (username LIKE ? OR nickname LIKE ?) ORDER BY created_at DESC LIMIT ? OFFSET ?"
            )
            .bind(tid)
            .bind(&pattern)
            .bind(&pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.state.db)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, nickname FROM iam_users WHERE tenant_id = ? AND deleted_at IS NULL AND status = 'active' ORDER BY created_at DESC LIMIT ? OFFSET ?"
            )
            .bind(tid)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.state.db)
            .await?
        };

        let total: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM iam_users WHERE tenant_id = ? AND deleted_at IS NULL AND status = 'active'"
        )
        .bind(tid)
        .fetch_one(&self.state.db)
        .await?;

        Ok(UserSimpleListResponse {
            items: users,
            total: total.0 as u64,
            page: query.page,
            size: query.size,
        })
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

    pub async fn list_by_role(
        &self,
        role_id: &str,
        page: u32,
        size: u32,
        scope: &TenantScope,
    ) -> Result<(Vec<String>, i64, bool), IamError> {
        let size = size.clamp(1, 1000);
        let page = page.max(1);
        let offset = ((page - 1) * size) as i64;
        let limit = size as i64;
        let tid = scope.tenant_id().unwrap_or(1) as u64;

        let items: Vec<String> = sqlx::query_scalar(
            "SELECT u.id FROM iam_users u \
             JOIN iam_user_roles ur ON ur.user_id = u.id \
             WHERE ur.role_id = ? AND u.deleted_at IS NULL AND u.tenant_id = ? \
             ORDER BY u.id LIMIT ? OFFSET ?",
        )
        .bind(role_id)
        .bind(tid)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.state.db)
        .await?;

        let total: (i64,) = sqlx::query_as(
            "SELECT COUNT(DISTINCT u.id) FROM iam_users u \
             JOIN iam_user_roles ur ON ur.user_id = u.id \
             WHERE ur.role_id = ? AND u.deleted_at IS NULL AND u.tenant_id = ?",
        )
        .bind(role_id)
        .bind(tid)
        .fetch_one(&self.state.db)
        .await?;

        let truncated = items.len() as u32 == 1000;
        Ok((items, total.0, truncated))
    }

    pub async fn list_by_dept(
        &self,
        dept_id: &str,
        page: u32,
        size: u32,
        scope: &TenantScope,
    ) -> Result<(Vec<String>, i64, bool), IamError> {
        let size = size.clamp(1, 1000);
        let page = page.max(1);
        let offset = ((page - 1) * size) as i64;
        let limit = size as i64;
        let tid = scope.tenant_id().unwrap_or(1) as u64;

        // Schema amendment (2026-07-01): iam uses many-to-many
        // iam_user_org_units(user_id, org_unit_id, is_primary). Loosest semantics
        // chosen: return users with ANY membership in the dept.
        let items: Vec<String> = sqlx::query_scalar(
            "SELECT u.id FROM iam_users u \
             JOIN iam_user_org_units uox ON uox.user_id = u.id \
             WHERE uox.org_unit_id = ? AND u.deleted_at IS NULL AND u.tenant_id = ? \
             GROUP BY u.id ORDER BY u.id LIMIT ? OFFSET ?",
        )
        .bind(dept_id)
        .bind(tid)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.state.db)
        .await?;

        let total: (i64,) = sqlx::query_as(
            "SELECT COUNT(DISTINCT u.id) FROM iam_users u \
             JOIN iam_user_org_units uox ON uox.user_id = u.id \
             WHERE uox.org_unit_id = ? AND u.deleted_at IS NULL AND u.tenant_id = ?",
        )
        .bind(dept_id)
        .bind(tid)
        .fetch_one(&self.state.db)
        .await?;

        let truncated = items.len() as u32 == 1000;
        Ok((items, total.0, truncated))
    }

    pub async fn get_claims(&self, user_id: &str, scope: &TenantScope) -> Result<UserClaimsResponse, IamError> {
        let tenant_id = scope.tenant_id().unwrap_or(1);
        let tid = tenant_id as u64;

        let mut qb = QueryBuilder::new(
            "SELECT id, username, email, phone, nickname FROM iam_users",
        );
        qb.where_id(user_id);
        qb.and_tenant(scope);
        qb.push(" AND deleted_at IS NULL AND status = 'active'");
        let user: Option<(String, String, Option<String>, Option<String>, String)> = qb
            .build_query_as::<(String, String, Option<String>, Option<String>, String)>()
            .fetch_optional(&self.state.db)
            .await?;

        let (uid, username, email, phone, nickname) = match user {
            Some(u) => u,
            None => return Err(IamError::UserNotFound),
        };

        let roles: Vec<String> = sqlx::query_scalar(
            "SELECT r.code FROM iam_roles r JOIN iam_user_roles ur ON r.id = ur.role_id WHERE ur.user_id = ? AND r.tenant_id = ? AND r.deleted_at IS NULL"
        )
        .bind(user_id)
        .bind(tid)
        .fetch_all(&self.state.db)
        .await?;

        let permissions: Vec<String> = sqlx::query_scalar(
            "SELECT p.code FROM iam_permissions p JOIN iam_role_permissions rp ON p.id = rp.permission_id JOIN iam_user_roles ur ON rp.role_id = ur.role_id JOIN iam_roles r ON ur.role_id = r.id WHERE ur.user_id = ? AND r.tenant_id = ? AND p.deleted_at IS NULL"
        )
        .bind(user_id)
        .bind(tid)
        .fetch_all(&self.state.db)
        .await?;

        let cross_tenant = roles.iter().any(|r| r == "platform_admin");

        Ok(UserClaimsResponse {
            user_id: uid,
            username,
            nickname,
            email,
            phone,
            roles,
            permissions,
            tenant_id,
            cross_tenant,
        })
    }
}
