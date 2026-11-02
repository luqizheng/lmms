use crate::{
    dto::{
        CreateUserRequest, UpdateUserRequest, UserClaimsResponse, UserListResponse, UserQuery,
        UserResponse, UserSimpleListResponse, UserSimpleQuery, UserSimpleResponse,
    },
    error::IamError,
    state::AppState,
};
use chrono::Utc;
use digua_common_crypto::password::hash;
use digua_common_db::{db_execute, db_fetch_all, db_fetch_one, db_fetch_optional, WithTenantScope};
use digua_common_event::events::{IamUserCreated, IamUserDeleted};
use digua_common_event::{AnyTx, Envelope, OutboxRepo};
use digua_jwt_ctx::TenantScope;
use nanoid::nanoid;
use sqlx::{FromRow, QueryBuilder};

/// `UserRow` 全列 SELECT 片段（双臂子函数共用；FromRow 按列名取值，两侧列集一致）。
const USER_COLS: &str = "id, tenant_id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at";

#[derive(FromRow)]
#[allow(dead_code)] // sqlx FromRow deserializes all selected columns; unused ones are filtered by SELECT.
struct UserRow {
    id: String,
    /// tenant_id 统一有符号 i64（PG 列即 BIGINT；MySQL UNSIGNED 列存正数无损，S5d）。
    tenant_id: i64,
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

/// Subset of `iam_users` columns used by `UserService::get_claims`.
/// Aliased here so callers don't have to read a bare 5-tuple type.
#[derive(FromRow)]
struct UserClaimsRow {
    id: String,
    username: String,
    email: Option<String>,
    phone: Option<String>,
    nickname: String,
}

pub struct UserService {
    state: AppState,
}

impl UserService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(
        &self,
        req: CreateUserRequest,
        scope: &TenantScope,
    ) -> Result<UserResponse, IamError> {
        // tenant_id 统一按有符号 i64 绑定（PG 列即 BIGINT；MySQL UNSIGNED 列存正数无损）。
        let tenant_id = scope.tenant_id().unwrap_or(1);

        let exists: Option<(String,)> = db_fetch_optional!(
            &self.state.db,
            (String,),
            "SELECT id FROM iam_users WHERE username = ? AND tenant_id = ? AND deleted_at IS NULL",
            &req.username,
            tenant_id
        )?;

        if exists.is_some() {
            return Err(IamError::DuplicateUser);
        }

        let password_hash = hash(&req.password).map_err(|e| IamError::Jwt(e.to_string()))?;

        let user_id = nanoid!();
        let salt = nanoid!(32);

        let (user_count,): (i64,) = db_fetch_one!(
            &self.state.db,
            (i64,),
            "SELECT COUNT(*) FROM iam_users WHERE deleted_at IS NULL"
        )?;
        let mut role_ids = req.role_ids.clone().unwrap_or_default();
        if user_count == 0 {
            // V003 起迁移会播种 (tenant 1, 'tenant_admin') 全局共享角色；首用户
            // 逻辑改为查存在优先复用，避免唯一键冲突 500（下游联调实测）。
            let existing: Option<(String,)> = db_fetch_optional!(
                &self.state.db,
                (String,),
                "SELECT id FROM iam_roles WHERE tenant_id = ? AND code = 'tenant_admin' \
                 AND deleted_at IS NULL LIMIT 1",
                tenant_id
            )?;
            let role_id = match existing {
                Some((id,)) => id,
                None => {
                    let role_id = nanoid!();
                    db_execute!(
                        &self.state.db,
                        "INSERT INTO iam_roles (id, tenant_id, name, code, description, data_scope, status, created_at, updated_at) \
                         VALUES (?, ?, '租户管理员', 'tenant_admin', '首用户自动创建', 'all', 'active', NOW(), NOW())",
                        &role_id,
                        tenant_id
                    )?;
                    role_id
                }
            };
            // 权限绑定幂等补齐（复用的种子角色可能尚未绑定任何权限）。
            db_execute!(
                &self.state.db,
                "INSERT INTO iam_role_permissions (role_id, permission_id, created_at) \
                 SELECT ?, id, NOW() FROM iam_permissions \
                 WHERE status = 'enabled' AND deleted_at IS NULL \
                   AND id NOT IN (SELECT permission_id FROM iam_role_permissions WHERE role_id = ?)",
                &role_id,
                &role_id
            )?;
            role_ids.push(role_id);
        }

        // 用户 INSERT + IamUserCreated outbox + 首角色绑定同事务 → 双臂（prue 样板）。
        match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                sqlx::query(
                    "INSERT INTO iam_users (id, tenant_id, username, password_hash, salt, nickname, email, phone, avatar, status, email_verified, phone_verified, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', false, false, NOW(), NOW())"
                )
                .bind(&user_id)
                .bind(tenant_id)
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
                self.state
                    .outbox_repo
                    .enqueue(&mut AnyTx::Mysql(&mut tx), &mut envelope)
                    .await
                    .map_err(|e| IamError::Internal(e.to_string()))?;

                crate::service::user_roles::set_user_roles_in_tx(
                    &mut AnyTx::Mysql(&mut tx),
                    &user_id,
                    &role_ids,
                )
                .await?;
                let ts = Utc::now().timestamp();
                for role_id in &role_ids {
                    crate::service::membership_event::enqueue_role_assigned(
                        &mut AnyTx::Mysql(&mut tx),
                        self.state.outbox_repo.as_ref(),
                        &user_id,
                        role_id,
                        tenant_id,
                        None,
                        ts,
                    )
                    .await?;
                }

                tx.commit().await?;
            }
            // PG 臂：$N 编号占位符；NOW()/false 两侧通用（PG 大小写不敏感）。
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                sqlx::query(
                    "INSERT INTO iam_users (id, tenant_id, username, password_hash, salt, nickname, email, phone, avatar, status, email_verified, phone_verified, created_at, updated_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'active', false, false, NOW(), NOW())"
                )
                .bind(&user_id)
                .bind(tenant_id)
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
                self.state
                    .outbox_repo
                    .enqueue(&mut AnyTx::Postgres(&mut tx), &mut envelope)
                    .await
                    .map_err(|e| IamError::Internal(e.to_string()))?;

                crate::service::user_roles::set_user_roles_in_tx(
                    &mut AnyTx::Postgres(&mut tx),
                    &user_id,
                    &role_ids,
                )
                .await?;
                let ts = Utc::now().timestamp();
                for role_id in &role_ids {
                    crate::service::membership_event::enqueue_role_assigned(
                        &mut AnyTx::Postgres(&mut tx),
                        self.state.outbox_repo.as_ref(),
                        &user_id,
                        role_id,
                        tenant_id,
                        None,
                        ts,
                    )
                    .await?;
                }

                tx.commit().await?;
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "user create tx: dialect feature not enabled".into(),
                ))
            }
        }

        // 提交后回读（QueryBuilder 方言泛型 → 双臂子函数；新建行未删，免 deleted_at 过滤）。
        let user = match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                fetch_user_mysql(p, &user_id, scope, false).await?
            }
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                fetch_user_pg(p, &user_id, scope, false).await?
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "user create readback: dialect feature not enabled".into(),
                ))
            }
        };
        let user =
            user.ok_or_else(|| IamError::Internal("user create: post-commit row missing".into()))?;

        Ok(self.to_response(&user))
    }

    pub async fn get(&self, user_id: &str, scope: &TenantScope) -> Result<UserResponse, IamError> {
        // QueryBuilder 方言泛型（push_bind 产出方言占位符）→ 双臂子函数（prue 样板）。
        let user = match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => fetch_user_mysql(p, user_id, scope, true).await?,
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => fetch_user_pg(p, user_id, scope, true).await?,
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "user get: dialect feature not enabled".into(),
                ))
            }
        };

        match user {
            Some(u) => Ok(self.to_response(&u)),
            None => Err(IamError::UserNotFound),
        }
    }

    pub async fn list(
        &self,
        query: UserQuery,
        scope: &TenantScope,
    ) -> Result<UserListResponse, IamError> {
        let offset = ((query.page - 1) * query.size) as i64;
        let limit = query.size as i64;
        let tid = scope.tenant_id().unwrap_or(1);

        let users: Vec<UserRow> = if let Some(keyword) = &query.keyword {
            let pattern = format!("%{}%", keyword);
            db_fetch_all!(
                &self.state.db,
                UserRow,
                "SELECT id, tenant_id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM iam_users WHERE tenant_id = ? AND deleted_at IS NULL AND (username LIKE ? OR nickname LIKE ? OR email LIKE ?) ORDER BY created_at DESC LIMIT ? OFFSET ?",
                tid,
                &pattern,
                &pattern,
                &pattern,
                limit,
                offset
            )?
        } else {
            db_fetch_all!(
                &self.state.db,
                UserRow,
                "SELECT id, tenant_id, username, email, phone, password_hash, salt, nickname, avatar, status, email_verified, phone_verified, last_login_at, last_login_ip, password_expired_at, created_at, updated_at, deleted_at FROM iam_users WHERE tenant_id = ? AND deleted_at IS NULL ORDER BY created_at DESC LIMIT ? OFFSET ?",
                tid,
                limit,
                offset
            )?
        };

        let (total,): (i64,) = db_fetch_one!(
            &self.state.db,
            (i64,),
            "SELECT COUNT(*) FROM iam_users WHERE tenant_id = ? AND deleted_at IS NULL",
            tid
        )?;

        Ok(UserListResponse {
            items: users.into_iter().map(|u| self.to_response(&u)).collect(),
            total: total as u64,
            page: query.page,
            size: query.size,
        })
    }

    pub async fn update(
        &self,
        user_id: &str,
        req: UpdateUserRequest,
        scope: &TenantScope,
    ) -> Result<UserResponse, IamError> {
        let existing = match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => fetch_user_mysql(p, user_id, scope, true).await?,
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => fetch_user_pg(p, user_id, scope, true).await?,
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "user update: dialect feature not enabled".into(),
                ))
            }
        };

        let existing = match existing {
            Some(u) => u,
            None => return Err(IamError::UserNotFound),
        };

        let nickname = req.nickname.unwrap_or(existing.nickname);
        let email = req.email.or(existing.email);
        let phone = req.phone.or(existing.phone);
        let avatar = req.avatar.or(existing.avatar);
        let status = req.status.unwrap_or(existing.status);

        // 池级 QueryBuilder UPDATE → 双臂子函数（push_bind 自动产出方言占位符）。
        match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                update_profile_mysql(
                    p, user_id, &nickname, &email, &phone, &avatar, &status, scope,
                )
                .await?
            }
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                update_profile_pg(
                    p, user_id, &nickname, &email, &phone, &avatar, &status, scope,
                )
                .await?
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "user update write: dialect feature not enabled".into(),
                ))
            }
        };

        let user = match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => fetch_user_mysql(p, user_id, scope, true).await?,
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => fetch_user_pg(p, user_id, scope, true).await?,
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "user update readback: dialect feature not enabled".into(),
                ))
            }
        };
        let user = user.ok_or(IamError::UserNotFound)?;

        Ok(self.to_response(&user))
    }

    pub async fn delete(&self, user_id: &str, scope: &TenantScope) -> Result<(), IamError> {
        // tenant_id 统一 i64（PG 列即 BIGINT；MySQL UNSIGNED 列存正数无损）。
        let tenant_id = scope.tenant_id().unwrap_or(1);

        // 软删 + IamUserDeleted outbox 同事务 → 双臂（prue 样板）。
        match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                let mut qb = QueryBuilder::<sqlx::MySql>::new(
                    "UPDATE iam_users SET deleted_at = NOW(), updated_at = NOW()",
                );
                qb.where_id(user_id);
                qb.and_tenant(scope);
                qb.push(" AND deleted_at IS NULL");
                let result = qb.build().execute(&mut *tx).await?;

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
                self.state
                    .outbox_repo
                    .enqueue(&mut AnyTx::Mysql(&mut tx), &mut envelope)
                    .await
                    .map_err(|e| IamError::Internal(e.to_string()))?;
                tx.commit().await?;
            }
            // PG 臂：与 mysql 臂同构（QueryBuilder 泛型差异只在类型标注）。
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                let mut qb = QueryBuilder::<sqlx::Postgres>::new(
                    "UPDATE iam_users SET deleted_at = NOW(), updated_at = NOW()",
                );
                qb.where_id(user_id);
                qb.and_tenant(scope);
                qb.push(" AND deleted_at IS NULL");
                let result = qb.build().execute(&mut *tx).await?;

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
                self.state
                    .outbox_repo
                    .enqueue(&mut AnyTx::Postgres(&mut tx), &mut envelope)
                    .await
                    .map_err(|e| IamError::Internal(e.to_string()))?;
                tx.commit().await?;
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "user delete: dialect feature not enabled".into(),
                ))
            }
        }

        Ok(())
    }

    pub async fn list_simple(
        &self,
        query: UserSimpleQuery,
        scope: &TenantScope,
    ) -> Result<UserSimpleListResponse, IamError> {
        let offset = ((query.page - 1) * query.size) as i64;
        let limit = query.size as i64;
        let tid = scope.tenant_id().unwrap_or(1);

        let users: Vec<UserSimpleResponse> = if let Some(keyword) = &query.keyword {
            let pattern = format!("%{}%", keyword);
            db_fetch_all!(
                &self.state.db,
                UserSimpleResponse,
                "SELECT id, nickname FROM iam_users WHERE tenant_id = ? AND deleted_at IS NULL AND status = 'active' AND (username LIKE ? OR nickname LIKE ?) ORDER BY created_at DESC LIMIT ? OFFSET ?",
                tid,
                &pattern,
                &pattern,
                limit,
                offset
            )?
        } else {
            db_fetch_all!(
                &self.state.db,
                UserSimpleResponse,
                "SELECT id, nickname FROM iam_users WHERE tenant_id = ? AND deleted_at IS NULL AND status = 'active' ORDER BY created_at DESC LIMIT ? OFFSET ?",
                tid,
                limit,
                offset
            )?
        };

        let (total,): (i64,) = db_fetch_one!(
            &self.state.db,
            (i64,),
            "SELECT COUNT(*) FROM iam_users WHERE tenant_id = ? AND deleted_at IS NULL AND status = 'active'",
            tid
        )?;

        Ok(UserSimpleListResponse {
            items: users,
            total: total as u64,
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

    pub async fn batch_get_by_ids(
        &self,
        user_ids: &[String],
        scope: &TenantScope,
    ) -> Result<Vec<UserResponse>, IamError> {
        if user_ids.is_empty() {
            return Ok(vec![]);
        }

        // 动态 IN 列表的 QueryBuilder → 双臂子函数（separated/push_bind 由方言泛型展开）。
        let users: Vec<UserRow> = match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                fetch_users_by_ids_mysql(p, user_ids, scope).await?
            }
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                fetch_users_by_ids_pg(p, user_ids, scope).await?
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "batch_get_by_ids: dialect feature not enabled".into(),
                ))
            }
        };
        Ok(users.iter().map(|u| self.to_response(u)).collect())
    }

    pub async fn list_by_org_and_role(
        &self,
        org_unit_id: &str,
        role_id: &str,
        scope: &TenantScope,
    ) -> Result<Vec<UserResponse>, IamError> {
        let tid = scope.tenant_id().unwrap_or(1);
        let users: Vec<UserRow> = db_fetch_all!(
            &self.state.db,
            UserRow,
            "SELECT u.id, u.tenant_id, u.username, u.email, u.phone, u.password_hash, u.salt, \
             u.nickname, u.avatar, u.status, u.email_verified, u.phone_verified, \
             u.last_login_at, u.last_login_ip, u.password_expired_at, u.created_at, u.updated_at, u.deleted_at \
             FROM iam_users u \
             JOIN iam_user_org_units uox ON uox.user_id = u.id \
             JOIN iam_user_roles ur ON ur.user_id = u.id \
             WHERE uox.org_unit_id = ? AND ur.role_id = ? AND u.tenant_id = ? AND u.deleted_at IS NULL",
            org_unit_id,
            role_id,
            tid
        )?;
        Ok(users.iter().map(|u| self.to_response(u)).collect())
    }

    pub async fn list_users_by_role_with_details(
        &self,
        role_id: &str,
        page: u32,
        size: u32,
        scope: &TenantScope,
    ) -> Result<(Vec<UserResponse>, i64), IamError> {
        let size = size.clamp(1, 100);
        let page = page.max(1);
        let offset = ((page - 1) * size) as i64;
        let limit = size as i64;
        let tid = scope.tenant_id().unwrap_or(1);

        let users: Vec<UserRow> = db_fetch_all!(
            &self.state.db,
            UserRow,
            "SELECT u.id, u.tenant_id, u.username, u.email, u.phone, u.password_hash, u.salt, \
             u.nickname, u.avatar, u.status, u.email_verified, u.phone_verified, \
             u.last_login_at, u.last_login_ip, u.password_expired_at, u.created_at, u.updated_at, u.deleted_at \
             FROM iam_users u \
             JOIN iam_user_roles ur ON ur.user_id = u.id \
             WHERE ur.role_id = ? AND u.deleted_at IS NULL AND u.tenant_id = ? \
             ORDER BY u.id LIMIT ? OFFSET ?",
            role_id,
            tid,
            limit,
            offset
        )?;

        let (total,): (i64,) = db_fetch_one!(
            &self.state.db,
            (i64,),
            "SELECT COUNT(DISTINCT u.id) FROM iam_users u \
             JOIN iam_user_roles ur ON ur.user_id = u.id \
             WHERE ur.role_id = ? AND u.deleted_at IS NULL AND u.tenant_id = ?",
            role_id,
            tid
        )?;

        let items = users.iter().map(|u| self.to_response(u)).collect();
        Ok((items, total))
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
        let tid = scope.tenant_id().unwrap_or(1);

        let items: Vec<String> = db_fetch_all!(
            &self.state.db,
            (String,),
            "SELECT u.id FROM iam_users u \
             JOIN iam_user_roles ur ON ur.user_id = u.id \
             WHERE ur.role_id = ? AND u.deleted_at IS NULL AND u.tenant_id = ? \
             ORDER BY u.id LIMIT ? OFFSET ?",
            role_id,
            tid,
            limit,
            offset
        )?
        .into_iter()
        .map(|(id,)| id)
        .collect();

        let (total,): (i64,) = db_fetch_one!(
            &self.state.db,
            (i64,),
            "SELECT COUNT(DISTINCT u.id) FROM iam_users u \
             JOIN iam_user_roles ur ON ur.user_id = u.id \
             WHERE ur.role_id = ? AND u.deleted_at IS NULL AND u.tenant_id = ?",
            role_id,
            tid
        )?;

        let truncated = items.len() as u32 == 1000;
        Ok((items, total, truncated))
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
        let tid = scope.tenant_id().unwrap_or(1);

        // Schema amendment (2026-07-01): iam uses many-to-many
        // iam_user_org_units(user_id, org_unit_id, is_primary). Loosest semantics
        // chosen: return users with ANY membership in the dept.
        let items: Vec<String> = db_fetch_all!(
            &self.state.db,
            (String,),
            "SELECT u.id FROM iam_users u \
             JOIN iam_user_org_units uox ON uox.user_id = u.id \
             WHERE uox.org_unit_id = ? AND u.deleted_at IS NULL AND u.tenant_id = ? \
             GROUP BY u.id ORDER BY u.id LIMIT ? OFFSET ?",
            dept_id,
            tid,
            limit,
            offset
        )?
        .into_iter()
        .map(|(id,)| id)
        .collect();

        let (total,): (i64,) = db_fetch_one!(
            &self.state.db,
            (i64,),
            "SELECT COUNT(DISTINCT u.id) FROM iam_users u \
             JOIN iam_user_org_units uox ON uox.user_id = u.id \
             WHERE uox.org_unit_id = ? AND u.deleted_at IS NULL AND u.tenant_id = ?",
            dept_id,
            tid
        )?;

        let truncated = items.len() as u32 == 1000;
        Ok((items, total, truncated))
    }

    pub async fn get_claims(
        &self,
        user_id: &str,
        scope: &TenantScope,
    ) -> Result<UserClaimsResponse, IamError> {
        // tenant_id 统一 i64：bind 与响应字段（dto tid）同型。
        let tenant_id = scope.tenant_id().unwrap_or(1);

        let user = match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => fetch_claims_mysql(p, user_id, scope).await?,
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => fetch_claims_pg(p, user_id, scope).await?,
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "get_claims: dialect feature not enabled".into(),
                ))
            }
        };

        let UserClaimsRow {
            id: uid,
            username,
            email,
            phone,
            nickname,
        } = user.ok_or(IamError::UserNotFound)?;

        let roles: Vec<String> = db_fetch_all!(
            &self.state.db,
            (String,),
            "SELECT r.code FROM iam_roles r JOIN iam_user_roles ur ON r.id = ur.role_id WHERE ur.user_id = ? AND r.tenant_id = ? AND r.deleted_at IS NULL",
            user_id,
            tenant_id
        )?
        .into_iter()
        .map(|(code,)| code)
        .collect();

        let permissions: Vec<String> = db_fetch_all!(
            &self.state.db,
            (String,),
            "SELECT p.code FROM iam_permissions p JOIN iam_role_permissions rp ON p.id = rp.permission_id JOIN iam_user_roles ur ON rp.role_id = ur.role_id JOIN iam_roles r ON ur.role_id = r.id WHERE ur.user_id = ? AND r.tenant_id = ? AND p.deleted_at IS NULL",
            user_id,
            tenant_id
        )?
        .into_iter()
        .map(|(code,)| code)
        .collect();

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

/// 双臂共用的用户行读取（QueryBuilder + and_tenant）；`live_only` 控制
/// 是否追加 `AND deleted_at IS NULL`（create 提交后回读新行时免过滤）。
#[cfg(feature = "mysql")]
async fn fetch_user_mysql(
    pool: &sqlx::MySqlPool,
    user_id: &str,
    scope: &TenantScope,
    live_only: bool,
) -> Result<Option<UserRow>, IamError> {
    let mut qb = QueryBuilder::<sqlx::MySql>::new(format!("SELECT {USER_COLS} FROM iam_users"));
    qb.where_id(user_id);
    qb.and_tenant(scope);
    if live_only {
        qb.push(" AND deleted_at IS NULL");
    }
    Ok(qb.build_query_as::<UserRow>().fetch_optional(pool).await?)
}

/// PG 臂：与 mysql 臂同构（QueryBuilder 泛型差异只在类型标注）。
#[cfg(feature = "postgres")]
async fn fetch_user_pg(
    pool: &sqlx::PgPool,
    user_id: &str,
    scope: &TenantScope,
    live_only: bool,
) -> Result<Option<UserRow>, IamError> {
    let mut qb = QueryBuilder::<sqlx::Postgres>::new(format!("SELECT {USER_COLS} FROM iam_users"));
    qb.where_id(user_id);
    qb.and_tenant(scope);
    if live_only {
        qb.push(" AND deleted_at IS NULL");
    }
    Ok(qb.build_query_as::<UserRow>().fetch_optional(pool).await?)
}

/// 池级资料 UPDATE（mysql 臂）；结果丢弃，调用方另行回读。
#[cfg(feature = "mysql")]
#[allow(clippy::too_many_arguments)] // 行内字段逐个 bind，聚合结构体反而遮蔽 SQL 形状。
async fn update_profile_mysql(
    pool: &sqlx::MySqlPool,
    user_id: &str,
    nickname: &str,
    email: &Option<String>,
    phone: &Option<String>,
    avatar: &Option<String>,
    status: &str,
    scope: &TenantScope,
) -> Result<(), IamError> {
    let mut qb = QueryBuilder::<sqlx::MySql>::new("UPDATE iam_users SET nickname = ");
    qb.push_bind(nickname);
    qb.push(", email = ");
    qb.push_bind(email);
    qb.push(", phone = ");
    qb.push_bind(phone);
    qb.push(", avatar = ");
    qb.push_bind(avatar);
    qb.push(", status = ");
    qb.push_bind(status);
    // NOW() 两侧通用（PG 是大小写不敏感别名）。
    qb.push(", updated_at = NOW()");
    qb.where_id(user_id);
    qb.and_tenant(scope);
    qb.push(" AND deleted_at IS NULL");
    qb.build().execute(pool).await?;
    Ok(())
}

/// PG 臂：与 mysql 臂同构（QueryBuilder 泛型差异只在类型标注）。
#[cfg(feature = "postgres")]
#[allow(clippy::too_many_arguments)] // 行内字段逐个 bind，聚合结构体反而遮蔽 SQL 形状。
async fn update_profile_pg(
    pool: &sqlx::PgPool,
    user_id: &str,
    nickname: &str,
    email: &Option<String>,
    phone: &Option<String>,
    avatar: &Option<String>,
    status: &str,
    scope: &TenantScope,
) -> Result<(), IamError> {
    let mut qb = QueryBuilder::<sqlx::Postgres>::new("UPDATE iam_users SET nickname = ");
    qb.push_bind(nickname);
    qb.push(", email = ");
    qb.push_bind(email);
    qb.push(", phone = ");
    qb.push_bind(phone);
    qb.push(", avatar = ");
    qb.push_bind(avatar);
    qb.push(", status = ");
    qb.push_bind(status);
    qb.push(", updated_at = NOW()");
    qb.where_id(user_id);
    qb.and_tenant(scope);
    qb.push(" AND deleted_at IS NULL");
    qb.build().execute(pool).await?;
    Ok(())
}

/// 动态 IN 列表读取（mysql 臂，separated/push_bind 由 QueryBuilder 方言化）。
#[cfg(feature = "mysql")]
async fn fetch_users_by_ids_mysql(
    pool: &sqlx::MySqlPool,
    user_ids: &[String],
    scope: &TenantScope,
) -> Result<Vec<UserRow>, IamError> {
    let mut qb = QueryBuilder::<sqlx::MySql>::new(format!("SELECT {USER_COLS} FROM iam_users"));
    qb.push(" WHERE id IN (");
    let mut separated = qb.separated(", ");
    for id in user_ids {
        separated.push_bind(id);
    }
    separated.push_unseparated(")");
    qb.and_tenant(scope);
    qb.push(" AND deleted_at IS NULL");
    Ok(qb.build_query_as::<UserRow>().fetch_all(pool).await?)
}

/// PG 臂：与 mysql 臂同构（泛型仅差类型参数，不能与 mysql 版共用一个签名）。
#[cfg(feature = "postgres")]
async fn fetch_users_by_ids_pg(
    pool: &sqlx::PgPool,
    user_ids: &[String],
    scope: &TenantScope,
) -> Result<Vec<UserRow>, IamError> {
    let mut qb = QueryBuilder::<sqlx::Postgres>::new(format!("SELECT {USER_COLS} FROM iam_users"));
    qb.push(" WHERE id IN (");
    let mut separated = qb.separated(", ");
    for id in user_ids {
        separated.push_bind(id);
    }
    separated.push_unseparated(")");
    qb.and_tenant(scope);
    qb.push(" AND deleted_at IS NULL");
    Ok(qb.build_query_as::<UserRow>().fetch_all(pool).await?)
}

/// get_claims 的用户子集读取（mysql 臂）。
#[cfg(feature = "mysql")]
async fn fetch_claims_mysql(
    pool: &sqlx::MySqlPool,
    user_id: &str,
    scope: &TenantScope,
) -> Result<Option<UserClaimsRow>, IamError> {
    let mut qb = QueryBuilder::<sqlx::MySql>::new(
        "SELECT id, username, email, phone, nickname FROM iam_users",
    );
    qb.where_id(user_id);
    qb.and_tenant(scope);
    qb.push(" AND deleted_at IS NULL AND status = 'active'");
    Ok(qb
        .build_query_as::<UserClaimsRow>()
        .fetch_optional(pool)
        .await?)
}

/// PG 臂：与 mysql 臂同构（QueryBuilder 泛型差异只在类型标注）。
#[cfg(feature = "postgres")]
async fn fetch_claims_pg(
    pool: &sqlx::PgPool,
    user_id: &str,
    scope: &TenantScope,
) -> Result<Option<UserClaimsRow>, IamError> {
    let mut qb = QueryBuilder::<sqlx::Postgres>::new(
        "SELECT id, username, email, phone, nickname FROM iam_users",
    );
    qb.where_id(user_id);
    qb.and_tenant(scope);
    qb.push(" AND deleted_at IS NULL AND status = 'active'");
    Ok(qb
        .build_query_as::<UserClaimsRow>()
        .fetch_optional(pool)
        .await?)
}
