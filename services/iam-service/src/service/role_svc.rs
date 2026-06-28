use crate::{
    error::IamError,
    state::AppState,
    dto::{CreateRoleRequest, UpdateRoleRequest, RoleQuery, RoleResponse, RoleListResponse},
};
use chrono::Utc;
use nanoid::nanoid;
use sqlx::{FromRow, mysql::MySqlQueryResult};

pub struct RoleService {
    state: AppState,
}

#[derive(FromRow)]
struct RoleRow {
    id: String,
    name: String,
    code: String,
    description: Option<String>,
    data_scope: String,
    status: String,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
    deleted_at: Option<chrono::DateTime<Utc>>,
}

impl RoleService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(&self, req: CreateRoleRequest) -> Result<RoleResponse, IamError> {
        let role_id = nanoid!();
        let data_scope_str = data_scope_to_str(req.data_scope);

        sqlx::query(
            r#"INSERT INTO iam_roles (id, name, code, description, data_scope, status, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, 'active', NOW(), NOW())"#,
        )
        .bind(&role_id)
        .bind(&req.name)
        .bind(&req.code)
        .bind(&req.description)
        .bind(data_scope_str)
        .execute(&self.state.db)
        .await?;

        self.get(&role_id).await
    }

    pub async fn get(&self, role_id: &str) -> Result<RoleResponse, IamError> {
        let role: Option<RoleRow> = sqlx::query_as(
            "SELECT id, name, code, description, data_scope, status, created_at, updated_at, deleted_at FROM iam_roles WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(role_id)
        .fetch_optional(&self.state.db)
        .await?;

        match role {
            Some(r) => Ok(self.to_response(&r)),
            None => Err(IamError::RoleNotFound),
        }
    }

    pub async fn list(&self, query: RoleQuery) -> Result<RoleListResponse, IamError> {
        let offset = ((query.page - 1) * query.size) as i64;
        let limit = query.size as i64;

        let roles: Vec<RoleRow> = if let Some(keyword) = &query.keyword {
            let pattern = format!("%{}%", keyword);
            sqlx::query_as(
                "SELECT id, name, code, description, data_scope, status, created_at, updated_at, deleted_at FROM iam_roles WHERE deleted_at IS NULL AND (name LIKE ? OR code LIKE ?) ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(&pattern)
            .bind(&pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.state.db)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, name, code, description, data_scope, status, created_at, updated_at, deleted_at FROM iam_roles WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.state.db)
            .await?
        };

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM iam_roles WHERE deleted_at IS NULL")
            .fetch_one(&self.state.db)
            .await?;

        Ok(RoleListResponse {
            items: roles.into_iter().map(|r| self.to_response(&r)).collect(),
            total: count.0 as u64,
            page: query.page,
            size: query.size,
        })
    }

    pub async fn update(&self, role_id: &str, req: UpdateRoleRequest) -> Result<RoleResponse, IamError> {
        let role: Option<RoleRow> = sqlx::query_as(
            "SELECT id, name, code, description, data_scope, status, created_at, updated_at, deleted_at FROM iam_roles WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(role_id)
        .fetch_optional(&self.state.db)
        .await?;

        let role = match role {
            Some(r) => r,
            None => return Err(IamError::RoleNotFound),
        };

        let name = req.name.unwrap_or(role.name);
        let description = req.description.or(role.description);
        let data_scope_str = match req.data_scope {
            Some(n) => data_scope_to_str(n).to_string(),
            None => role.data_scope.clone(),
        };
        let status = req.status.unwrap_or(role.status.clone());

        // Fire iam.permission.changed only on a non-disabled → disabled
        // status transition (not on rename / other updates).
        let now_disabled = status == "disabled" && role.status != "disabled";
        let user_ids: Vec<String> = if now_disabled {
            sqlx::query_scalar("SELECT user_id FROM iam_user_roles WHERE role_id = ?")
                .bind(role_id)
                .fetch_all(&self.state.db)
                .await?
        } else {
            Vec::new()
        };

        let mut tx = self.state.db.begin().await?;
        sqlx::query(
            "UPDATE iam_roles SET name = ?, description = ?, data_scope = ?, status = ?, updated_at = NOW() WHERE id = ?",
        )
        .bind(&name)
        .bind(&description)
        .bind(&data_scope_str)
        .bind(&status)
        .bind(role_id)
        .execute(&mut *tx)
        .await?;

        if now_disabled {
            crate::service::perm_change::enqueue_permission_changed(
                &mut tx,
                self.state.outbox_repo.as_ref(),
                &user_ids,
                "role_disabled",
                None,
            )
            .await?;
        }
        tx.commit().await?;

        if now_disabled {
            crate::service::perm_change::stamp_perm_epochs(&self.state, &user_ids).await;
        }

        self.get(role_id).await
    }

    pub async fn delete(&self, role_id: &str) -> Result<(), IamError> {
        let user_ids: Vec<String> =
            sqlx::query_scalar("SELECT user_id FROM iam_user_roles WHERE role_id = ?")
                .bind(role_id)
                .fetch_all(&self.state.db)
                .await?;

        let mut tx = self.state.db.begin().await?;
        let result: MySqlQueryResult = sqlx::query(
            "UPDATE iam_roles SET deleted_at = NOW(), updated_at = NOW() WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(role_id)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(IamError::RoleNotFound);
        }

        crate::service::perm_change::enqueue_permission_changed(
            &mut tx,
            self.state.outbox_repo.as_ref(),
            &user_ids,
            "role_deleted",
            None,
        )
        .await?;
        tx.commit().await?;

        crate::service::perm_change::stamp_perm_epochs(&self.state, &user_ids).await;
        Ok(())
    }

    pub async fn assign_permissions(&self, role_id: &str, permission_ids: Vec<String>) -> Result<(), IamError> {
        let user_ids: Vec<String> =
            sqlx::query_scalar("SELECT user_id FROM iam_user_roles WHERE role_id = ?")
                .bind(role_id)
                .fetch_all(&self.state.db)
                .await?;

        let mut tx = self.state.db.begin().await?;
        sqlx::query("DELETE FROM iam_role_permissions WHERE role_id = ?")
            .bind(role_id)
            .execute(&mut *tx)
            .await?;
        for permission_id in &permission_ids {
            sqlx::query(
                "INSERT INTO iam_role_permissions (role_id, permission_id, created_at) VALUES (?, ?, NOW())",
            )
            .bind(role_id)
            .bind(permission_id)
            .execute(&mut *tx)
            .await?;
        }
        crate::service::perm_change::enqueue_permission_changed(
            &mut tx,
            self.state.outbox_repo.as_ref(),
            &user_ids,
            "role_permissions_changed",
            None,
        )
        .await?;
        tx.commit().await?;

        crate::service::perm_change::stamp_perm_epochs(&self.state, &user_ids).await;
        Ok(())
    }

    fn to_response(&self, role: &RoleRow) -> RoleResponse {
        RoleResponse {
            id: role.id.clone(),
            name: role.name.clone(),
            code: role.code.clone(),
            description: role.description.clone(),
            data_scope: role.data_scope.clone(),
            status: role.status.clone(),
            created_at: role.created_at,
            updated_at: role.updated_at,
        }
    }
}

/// 数据范围数字 -> 字符串。
/// 与前端 form.vue 的 dataScopeOptions 保持一致：
///   0=self、1=dept、2=dept_and_below、3=all
fn data_scope_to_str(n: i32) -> &'static str {
    match n {
        0 => "self",
        1 => "dept",
        2 => "dept_and_below",
        3 => "all",
        _ => "self",
    }
}
