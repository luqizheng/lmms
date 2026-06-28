use crate::{
    error::IamError,
    state::AppState,
    dto::{CreatePermissionRequest, UpdatePermissionRequest, PermissionQuery, PermissionResponse, PermissionListResponse},
};
use chrono::Utc;
use nanoid::nanoid;
use sqlx::{FromRow, mysql::MySqlQueryResult};

pub struct PermissionService {
    state: AppState,
}

#[derive(FromRow)]
struct PermissionRow {
    id: String,
    code: String,
    name: String,
    description: Option<String>,
    category: String,
    resource: Option<String>,
    action: Option<String>,
    sort_order: i32,
    status: String,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
    deleted_at: Option<chrono::DateTime<Utc>>,
}

impl PermissionService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(&self, req: CreatePermissionRequest) -> Result<PermissionResponse, IamError> {
        let permission_id = nanoid!();
        let category = match req.category {
            0 => "menu",
            1 => "button",
            2 => "api",
            _ => "api",
        };

        let permission: PermissionRow = sqlx::query_as(
            "INSERT INTO iam_permissions (id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'enabled', NOW(), NOW()) RETURNING id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at",
        )
        .bind(&permission_id)
        .bind(&req.code)
        .bind(&req.name)
        .bind(&req.description)
        .bind(category)
        .bind(&req.resource)
        .bind(&req.action)
        .bind(req.sort_order.unwrap_or(0))
        .fetch_one(&self.state.db)
        .await?;

        Ok(self.to_response(&permission))
    }

    pub async fn get(&self, permission_id: &str) -> Result<PermissionResponse, IamError> {
        let permission: Option<PermissionRow> = sqlx::query_as(
            "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(permission_id)
        .fetch_optional(&self.state.db)
        .await?;

        match permission {
            Some(p) => Ok(self.to_response(&p)),
            None => Err(IamError::PermissionNotFound),
        }
    }

    pub async fn list(&self, query: PermissionQuery) -> Result<PermissionListResponse, IamError> {
        let offset = ((query.page - 1) * query.size) as i64;
        let limit = query.size as i64;

        let permissions: Vec<PermissionRow> = if let Some(keyword) = &query.keyword {
            let pattern = format!("%{}%", keyword);
            if let Some(category) = query.category {
                let cat_str = match category {
                    0 => "menu",
                    1 => "button",
                    2 => "api",
                    _ => "api",
                };
                sqlx::query_as(
                    "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE deleted_at IS NULL AND (code LIKE ? OR name LIKE ?) AND category = ? ORDER BY sort_order, created_at LIMIT ? OFFSET ?",
                )
                .bind(&pattern)
                .bind(&pattern)
                .bind(cat_str)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.state.db)
                .await?
            } else {
                sqlx::query_as(
                    "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE deleted_at IS NULL AND (code LIKE ? OR name LIKE ?) ORDER BY sort_order, created_at LIMIT ? OFFSET ?",
                )
                .bind(&pattern)
                .bind(&pattern)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.state.db)
                .await?
            }
        } else if let Some(category) = query.category {
            let cat_str = match category {
                0 => "menu",
                1 => "button",
                2 => "api",
                _ => "api",
            };
            sqlx::query_as(
                "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE deleted_at IS NULL AND category = ? ORDER BY sort_order, created_at LIMIT ? OFFSET ?",
            )
            .bind(cat_str)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.state.db)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE deleted_at IS NULL ORDER BY sort_order, created_at LIMIT ? OFFSET ?",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.state.db)
            .await?
        };

        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM iam_permissions WHERE deleted_at IS NULL")
            .fetch_one(&self.state.db)
            .await?;

        Ok(PermissionListResponse {
            items: permissions.into_iter().map(|p| self.to_response(&p)).collect(),
            total: total.0 as u64,
            page: query.page,
            size: query.size,
        })
    }

    pub async fn update(&self, permission_id: &str, req: UpdatePermissionRequest) -> Result<PermissionResponse, IamError> {
        let permission: Option<PermissionRow> = sqlx::query_as(
            "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(permission_id)
        .fetch_optional(&self.state.db)
        .await?;

        let permission = match permission {
            Some(p) => p,
            None => return Err(IamError::PermissionNotFound),
        };

        let name = req.name.unwrap_or(permission.name);
        let description = req.description.or(permission.description);
        let sort_order = req.sort_order.unwrap_or(permission.sort_order);
        let status = req.status.unwrap_or(permission.status.clone());

        // Fire iam.permission.changed only on a non-disabled → disabled
        // status transition (not on rename / other updates).
        let now_disabled = status == "disabled" && permission.status != "disabled";
        let user_ids: Vec<String> = if now_disabled {
            sqlx::query_scalar(
                "SELECT DISTINCT ur.user_id FROM iam_user_roles ur JOIN iam_role_permissions rp ON ur.role_id = rp.role_id WHERE rp.permission_id = ?",
            )
            .bind(permission_id)
            .fetch_all(&self.state.db)
            .await?
        } else {
            Vec::new()
        };

        let mut tx = self.state.db.begin().await?;
        let result: PermissionRow = sqlx::query_as(
            "UPDATE iam_permissions SET name = ?, description = ?, sort_order = ?, status = ?, updated_at = NOW() WHERE id = ? RETURNING id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at",
        )
        .bind(&name)
        .bind(&description)
        .bind(sort_order)
        .bind(&status)
        .bind(permission_id)
        .fetch_one(&mut *tx)
        .await?;

        if now_disabled {
            crate::service::perm_change::enqueue_permission_changed(
                &mut tx,
                &self.state.db,
                &user_ids,
                "permission_disabled",
                None,
            )
            .await?;
        }
        tx.commit().await?;

        if now_disabled {
            crate::service::perm_change::stamp_perm_epochs(&self.state, &user_ids).await;
        }

        Ok(self.to_response(&result))
    }

    pub async fn delete(&self, permission_id: &str) -> Result<(), IamError> {
        // Expand affected users via the join BEFORE we delete the
        // iam_role_permissions rows we are joining on.
        let user_ids: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT ur.user_id FROM iam_user_roles ur JOIN iam_role_permissions rp ON ur.role_id = rp.role_id WHERE rp.permission_id = ?",
        )
        .bind(permission_id)
        .fetch_all(&self.state.db)
        .await?;

        let mut tx = self.state.db.begin().await?;
        let result: MySqlQueryResult = sqlx::query(
            "UPDATE iam_permissions SET deleted_at = NOW(), updated_at = NOW() WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(permission_id)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(IamError::PermissionNotFound);
        }

        sqlx::query("DELETE FROM iam_role_permissions WHERE permission_id = ?")
            .bind(permission_id)
            .execute(&mut *tx)
            .await?;

        crate::service::perm_change::enqueue_permission_changed(
            &mut tx,
            &self.state.db,
            &user_ids,
            "permission_deleted",
            None,
        )
        .await?;
        tx.commit().await?;

        crate::service::perm_change::stamp_perm_epochs(&self.state, &user_ids).await;
        Ok(())
    }

    fn to_response(&self, permission: &PermissionRow) -> PermissionResponse {
        PermissionResponse {
            id: permission.id.clone(),
            code: permission.code.clone(),
            name: permission.name.clone(),
            description: permission.description.clone(),
            category: permission.category.clone(),
            resource: permission.resource.clone(),
            action: permission.action.clone(),
            sort_order: permission.sort_order,
            status: permission.status.clone(),
            created_at: permission.created_at,
            updated_at: permission.updated_at,
        }
    }
}
