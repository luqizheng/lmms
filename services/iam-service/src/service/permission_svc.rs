use crate::{
    error::IamError,
    models::{Permission},
    state::AppState,
    dto::{CreatePermissionRequest, UpdatePermissionRequest, PermissionQuery, PermissionResponse, PermissionListResponse},
};
use chrono::{DateTime, Utc};
use nanoid::nanoid;

pub struct PermissionService {
    state: AppState,
}

impl PermissionService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(&self, req: CreatePermissionRequest) -> Result<PermissionResponse, IamError> {
        let exists = sqlx::query!(
            "SELECT id FROM permissions WHERE code = ? AND deleted_at IS NULL",
            req.code
        )
        .fetch_optional(&self.state.db)
        .await?
        .is_some();

        if exists {
            return Err(IamError::DuplicatePermission);
        }

        let permission_id = nanoid!();
        let category = match req.category {
            0 => "menu",
            1 => "button",
            2 => "api",
            _ => "api",
        };

        let permission = sqlx::query_as!(
            Permission,
            "INSERT INTO permissions (id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'enabled', ?, ?) RETURNING *",
            permission_id,
            req.code,
            req.name,
            req.description,
            category,
            req.resource,
            req.action,
            req.sort_order.unwrap_or(0),
            Utc::now(),
            Utc::now()
        )
        .fetch_one(&self.state.db)
        .await?;

        Ok(self.to_response(&permission))
    }

    pub async fn get(&self, permission_id: &str) -> Result<PermissionResponse, IamError> {
        let permission = sqlx::query_as!(
            Permission,
            "SELECT * FROM permissions WHERE id = ? AND deleted_at IS NULL",
            permission_id
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::PermissionNotFound)?;

        Ok(self.to_response(&permission))
    }

    pub async fn list(&self, query: PermissionQuery) -> Result<PermissionListResponse, IamError> {
        let offset = (query.page - 1) * query.size;
        
        let mut query_str = "SELECT * FROM permissions WHERE deleted_at IS NULL";
        let mut params: Vec<sqlx::types::JsonValue> = vec![];

        if let Some(keyword) = &query.keyword {
            query_str += " AND (code LIKE ? OR name LIKE ?)";
            let like_pattern = format!("%{}%", keyword);
            params.extend(vec![like_pattern.clone().into(); 2]);
        }

        if let Some(category) = &query.category {
            let cat_str = match category {
                0 => "menu",
                1 => "button",
                2 => "api",
                _ => "api",
            };
            query_str += " AND category = ?";
            params.push(cat_str.into());
        }

        let count: u64 = sqlx::query_scalar!("SELECT COUNT(*) FROM permissions WHERE deleted_at IS NULL", params[..])
            .fetch_one(&self.state.db)
            .await?
            .unwrap_or(0) as u64;

        query_str += " ORDER BY sort_order, created_at LIMIT ? OFFSET ?";
        params.push(query.size.into());
        params.push(offset.into());

        let permissions = sqlx::query_as!(Permission, &query_str, params[..])
            .fetch_all(&self.state.db)
            .await?;

        Ok(PermissionListResponse {
            items: permissions.into_iter().map(|p| self.to_response(&p)).collect(),
            total: count,
            page: query.page,
            size: query.size,
        })
    }

    pub async fn update(&self, permission_id: &str, req: UpdatePermissionRequest) -> Result<PermissionResponse, IamError> {
        let mut update_fields = vec![];
        let mut params: Vec<sqlx::types::JsonValue> = vec![];

        if let Some(name) = &req.name {
            update_fields.push("name = ?");
            params.push(name.into());
        }
        if let Some(description) = &req.description {
            update_fields.push("description = ?");
            params.push(description.into());
        }
        if let Some(sort_order) = &req.sort_order {
            update_fields.push("sort_order = ?");
            params.push(sort_order.into());
        }
        if let Some(status) = &req.status {
            update_fields.push("status = ?");
            params.push(status.into());
        }

        if update_fields.is_empty() {
            return self.get(permission_id);
        }

        update_fields.push("updated_at = ?");
        params.push(Utc::now().into());
        params.push(permission_id.into());

        let query_str = format!("UPDATE permissions SET {} WHERE id = ? AND deleted_at IS NULL RETURNING *", update_fields.join(", "));

        let permission = sqlx::query_as!(Permission, &query_str, params[..])
            .fetch_optional(&self.state.db)
            .await?
            .ok_or(IamError::PermissionNotFound)?;

        Ok(self.to_response(&permission))
    }

    pub async fn delete(&self, permission_id: &str) -> Result<(), IamError> {
        let result = sqlx::query!(
            "UPDATE permissions SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
            Utc::now(),
            Utc::now(),
            permission_id
        )
        .execute(&self.state.db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(IamError::PermissionNotFound);
        }

        sqlx::query!(
            "DELETE FROM role_permissions WHERE permission_id = ?",
            permission_id
        )
        .execute(&self.state.db)
        .await?;

        Ok(())
    }

    fn to_response(&self, permission: &Permission) -> PermissionResponse {
        PermissionResponse {
            id: permission.id.clone(),
            code: permission.code.clone(),
            name: permission.name.clone(),
            description: permission.description.clone(),
            category: format!("{}", permission.category),
            resource: permission.resource.clone(),
            action: permission.action.clone(),
            sort_order: permission.sort_order,
            status: format!("{}", permission.status),
            created_at: permission.created_at,
            updated_at: permission.updated_at,
        }
    }
}
