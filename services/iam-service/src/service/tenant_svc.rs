use crate::{
    dto::{CreateTenantRequest, CreateTenantResponse},
    error::IamError,
    state::AppState,
};
use common_crypto::password::hash;
use common_event::events::IamTenantCreated;
use common_event::{Envelope, OutboxRepo};
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
        let mut tx = self.state.db.begin().await?;

        let now = chrono::Utc::now();
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
        .map_err(|e| {
            if let Some(db_err) = e.as_database_error() {
                if db_err.code().map(|c| c.into_owned()).as_deref() == Some("23000") {
                    return IamError::DuplicateTenant;
                }
            }
            IamError::from(e)
        })?;
        let tenant_id = tenant_res.last_insert_id();

        let admin_id = Uuid::new_v4().to_string();
        let password_hash =
            hash(&req.admin_password).map_err(|e| IamError::Internal(e.to_string()))?;
        let salt = nanoid::nanoid!(32);

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

        let role_id = nanoid::nanoid!();
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
            .enqueue(&mut tx, &mut envelope)
            .await
            .map_err(|e| IamError::Internal(e.to_string()))?;

        tx.commit().await?;

        Ok(CreateTenantResponse {
            tenant_id: tenant_id as i64,
            admin_user_id: admin_id,
        })
    }
}
