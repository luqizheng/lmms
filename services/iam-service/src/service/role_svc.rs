use crate::{
    error::IamError,
    state::AppState,
    dto::{CreateRoleRequest, UpdateRoleRequest, RoleQuery, RoleResponse, RoleListResponse},
};
use chrono::Utc;
use nanoid::nanoid;
use sqlx::{FromRow, Row};

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

        let role: RoleRow = sqlx::query_as(
            r#"INSERT INTO roles (id, name, code, description, data_scope, status, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, 'active', NOW(), NOW())
               RETURNING id, name, code, description, data_scope, status, created_at, updated_at, deleted_at"#,
        )
        .bind(&role_id)
        .bind(&req.name)
        .bind(&req.code)
        .bind(&req.description)
        .bind(&req.data_scope)
        .fetch_one(&self.state.db)
        .await?;

        for permission_id in req.permission_ids {
            let _ = sqlx::query(
                "INSERT INTO role_permissions (role_id, permission_id) VALUES (?, ?)",
            )
            .bind(&role_id)
            .bind(&permission_id)
            .execute(&self.state.db)
            .await;
        }

        Ok(self.to_response(&role))
    }

    pub async fn get(&self, role_id: &str) -> Result<RoleResponse, IamError> {
        let role: Option<RoleRow> = sqlx::query_as(
            "SELECT id, name, code, description, data_scope, status, created_at, updated_at, deleted_at FROM roles WHERE id = ? AND deleted_at IS NULL",
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
                "SELECT id, name, code, description, data_scope, status, created_at, updated_at, deleted_at FROM roles WHERE deleted_at IS NULL AND (name LIKE ? OR code LIKE ?) ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(&pattern)
            .bind(&pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.state.db)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, name, code, description, data_scope, status, created_at, updated_at, deleted_at FROM roles WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.state.db)
            .await?
        };

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM roles WHERE deleted_at IS NULL")
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
            "SELECT id, name, code, description, data_scope, status, created_at, updated_at, deleted_at FROM roles WHERE id = ? AND deleted_at IS NULL",
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
        let data_scope = req.data_scope.unwrap_or(role.data_scope);
        let status = req.status.unwrap_or(role.status);

        let result: RoleRow = sqlx::query_as(
            "UPDATE roles SET name = ?, description = ?, data_scope = ?, status = ?, updated_at = NOW() WHERE id = ? RETURNING id, name, code, description, data_scope, status, created_at, updated_at, deleted_at",
        )
        .bind(&name)
        .bind(&description)
        .bind(&data_scope)
        .bind(&status)
        .bind(role_id)
        .fetch_one(&self.state.db)
        .await?;

        Ok(self.to_response(&result))
    }

    pub async fn delete(&self, role_id: &str) -> Result<(), IamError> {
        let result = sqlx::query(
            "UPDATE roles SET deleted_at = NOW(), updated_at = NOW() WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(role_id)
        .execute(&self.state.db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(IamError::RoleNotFound);
        }

        Ok(())
    }

    pub async fn assign_permissions(&self, role_id: &str, permission_ids: Vec<String>) -> Result<(), IamError> {
        let _ = sqlx::query("DELETE FROM role_permissions WHERE role_id = ?")
            .bind(role_id)
            .execute(&self.state.db)
            .await;

        for permission_id in permission_ids {
            let _ = sqlx::query(
                "INSERT INTO role_permissions (role_id, permission_id) VALUES (?, ?)",
            )
            .bind(role_id)
            .bind(&permission_id)
            .execute(&self.state.db)
            .await;
        }

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
