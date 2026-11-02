use crate::{
    dto::{CreateTenantRequest, CreateTenantResponse},
    error::IamError,
    state::AppState,
};
use digua_common_crypto::password::hash;
use digua_common_event::events::IamTenantCreated;
use digua_common_event::{AnyTx, Envelope, OutboxRepo};
use uuid::Uuid;

pub struct TenantService {
    state: AppState,
}

impl TenantService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn register(
        &self,
        req: CreateTenantRequest,
    ) -> Result<CreateTenantResponse, IamError> {
        let now = chrono::Utc::now();
        let admin_id = Uuid::new_v4().to_string();
        let password_hash =
            hash(&req.admin_password).map_err(|e| IamError::Internal(e.to_string()))?;
        let salt = nanoid::nanoid!(32);
        let role_id = nanoid::nanoid!();

        // 租户注册全流程单事务：tenant INSERT（自增 id 两侧取回方式不同）+
        // admin user/role/绑定 + 事件 outbox → 双臂（prue 样板）。
        let tenant_id: i64 = match &self.state.db {
            #[cfg(feature = "mysql")]
            digua_common_db::DbPool::Mysql(p) => {
                let mut tx = p.begin().await?;
                let tenant_res = sqlx::query(
                    "INSERT INTO iam_tenants (slug, display_name, status, created_at, updated_at) \
                     VALUES (?, ?, 'active', ?, ?)",
                )
                .bind(&req.tenant_slug)
                .bind(&req.display_name)
                .bind(now)
                .bind(now)
                .execute(&mut *tx)
                .await
                .map_err(dup_tenant_mysql)?;
                // MySQL：AUTO_INCREMENT 自增 id 经 last_insert_id() 取回（u64 → i64）。
                let tenant_id = tenant_res.last_insert_id() as i64;

                sqlx::query(
                    "INSERT INTO iam_users (id, tenant_id, username, nickname, password_hash, salt, email, status, email_verified, phone_verified, created_at, updated_at) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, 'active', false, false, ?, ?)",
                )
                .bind(&admin_id)
                .bind(tenant_id)
                .bind(&req.admin_username)
                .bind(&req.admin_username)
                .bind(&password_hash)
                .bind(&salt)
                .bind(&req.admin_email)
                .bind(now)
                .bind(now)
                .execute(&mut *tx)
                .await?;

                sqlx::query(
                    "INSERT INTO iam_roles (id, tenant_id, name, code, description, data_scope, status, created_at, updated_at) \
                     VALUES (?, ?, '租户管理员', 'tenant_admin', '租户管理员（注册自动创建）', 'all', 'active', ?, ?)",
                )
                .bind(&role_id)
                .bind(tenant_id)
                .bind(now)
                .bind(now)
                .execute(&mut *tx)
                .await?;

                sqlx::query(
                    "INSERT INTO iam_role_permissions (role_id, permission_id, created_at) \
                     SELECT ?, id, NOW() FROM iam_permissions WHERE status = 'enabled' AND deleted_at IS NULL",
                )
                .bind(&role_id)
                .execute(&mut *tx)
                .await?;

                sqlx::query(
                    "INSERT INTO iam_user_roles (user_id, role_id, created_at) VALUES (?, ?, NOW())",
                )
                .bind(&admin_id)
                .bind(&role_id)
                .execute(&mut *tx)
                .await?;

                crate::service::membership_event::enqueue_role_assigned(
                    &mut AnyTx::Mysql(&mut tx),
                    self.state.outbox_repo.as_ref(),
                    &admin_id,
                    &role_id,
                    tenant_id,
                    None,
                    now.timestamp(),
                )
                .await?;

                let mut envelope = Envelope::new(
                    "iam-service",
                    IamTenantCreated {
                        tenant_id: tenant_id.to_string(),
                        slug: req.tenant_slug.clone(),
                        display_name: req.display_name.clone(),
                        admin_user_id: admin_id.clone(),
                    },
                )
                .with_tenant_id(tenant_id.to_string());
                self.state
                    .outbox_repo
                    .enqueue(&mut AnyTx::Mysql(&mut tx), &mut envelope)
                    .await
                    .map_err(|e| IamError::Internal(e.to_string()))?;

                tx.commit().await?;
                tenant_id
            }
            // PG 臂：$N 编号占位符；PG 无 last_insert_id()，IDENTITY 主键经
            // RETURNING id 取回（notification-service create_announcement 样板）。
            #[cfg(feature = "postgres")]
            digua_common_db::DbPool::Postgres(p) => {
                let mut tx = p.begin().await?;
                let (tenant_id,): (i64,) = sqlx::query_as(
                    "INSERT INTO iam_tenants (slug, display_name, status, created_at, updated_at) \
                     VALUES ($1, $2, 'active', $3, $4) RETURNING id",
                )
                .bind(&req.tenant_slug)
                .bind(&req.display_name)
                .bind(now)
                .bind(now)
                .fetch_one(&mut *tx)
                .await
                .map_err(dup_tenant_pg)?;

                sqlx::query(
                    "INSERT INTO iam_users (id, tenant_id, username, nickname, password_hash, salt, email, status, email_verified, phone_verified, created_at, updated_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, 'active', false, false, $8, $9)",
                )
                .bind(&admin_id)
                .bind(tenant_id)
                .bind(&req.admin_username)
                .bind(&req.admin_username)
                .bind(&password_hash)
                .bind(&salt)
                .bind(&req.admin_email)
                .bind(now)
                .bind(now)
                .execute(&mut *tx)
                .await?;

                sqlx::query(
                    "INSERT INTO iam_roles (id, tenant_id, name, code, description, data_scope, status, created_at, updated_at) \
                     VALUES ($1, $2, '租户管理员', 'tenant_admin', '租户管理员（注册自动创建）', 'all', 'active', $3, $4)",
                )
                .bind(&role_id)
                .bind(tenant_id)
                .bind(now)
                .bind(now)
                .execute(&mut *tx)
                .await?;

                sqlx::query(
                    "INSERT INTO iam_role_permissions (role_id, permission_id, created_at) \
                     SELECT $1, id, NOW() FROM iam_permissions WHERE status = 'enabled' AND deleted_at IS NULL",
                )
                .bind(&role_id)
                .execute(&mut *tx)
                .await?;

                sqlx::query(
                    "INSERT INTO iam_user_roles (user_id, role_id, created_at) VALUES ($1, $2, NOW())",
                )
                .bind(&admin_id)
                .bind(&role_id)
                .execute(&mut *tx)
                .await?;

                crate::service::membership_event::enqueue_role_assigned(
                    &mut AnyTx::Postgres(&mut tx),
                    self.state.outbox_repo.as_ref(),
                    &admin_id,
                    &role_id,
                    tenant_id,
                    None,
                    now.timestamp(),
                )
                .await?;

                let mut envelope = Envelope::new(
                    "iam-service",
                    IamTenantCreated {
                        tenant_id: tenant_id.to_string(),
                        slug: req.tenant_slug.clone(),
                        display_name: req.display_name.clone(),
                        admin_user_id: admin_id.clone(),
                    },
                )
                .with_tenant_id(tenant_id.to_string());
                self.state
                    .outbox_repo
                    .enqueue(&mut AnyTx::Postgres(&mut tx), &mut envelope)
                    .await
                    .map_err(|e| IamError::Internal(e.to_string()))?;

                tx.commit().await?;
                tenant_id
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(IamError::Internal(
                    "tenant register: dialect feature not enabled".into(),
                ))
            }
        };

        Ok(CreateTenantResponse {
            tenant_id,
            admin_user_id: admin_id,
        })
    }
}

/// 唯一键冲突（slug）→ DuplicateTenant：MySQL 错误码 23000。
#[cfg(feature = "mysql")]
fn dup_tenant_mysql(e: sqlx::Error) -> IamError {
    if let Some(db_err) = e.as_database_error() {
        if db_err.code().map(|c| c.into_owned()).as_deref() == Some("23000") {
            return IamError::DuplicateTenant;
        }
    }
    IamError::from(e)
}

/// PG 臂：unique violation 的 SQLSTATE 是 23505（非 MySQL 的 23000）。
#[cfg(feature = "postgres")]
fn dup_tenant_pg(e: sqlx::Error) -> IamError {
    if let Some(db_err) = e.as_database_error() {
        if db_err.code().map(|c| c.into_owned()).as_deref() == Some("23505") {
            return IamError::DuplicateTenant;
        }
    }
    IamError::from(e)
}
