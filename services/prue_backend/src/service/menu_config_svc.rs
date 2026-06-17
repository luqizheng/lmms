use crate::{
    dto::{
        validate_routes_json, CreateMenuConfigRequest, MenuConfigDetail, MenuConfigListResponse,
        MenuConfigSummary, UpdateMenuConfigRequest,
    },
    error::{AppError, AppResult},
    state::AppState,
};
use chrono::{DateTime, Utc};
use common_db::DbPool;
use nanoid::nanoid;
use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct MenuConfigRow {
    id: String,
    version: i64,
    name: String,
    description: Option<String>,
    routes_json: String,
    status: String,
    is_active: bool,
    created_by: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    published_at: Option<DateTime<Utc>>,
}

pub struct MenuConfigService {
    state: AppState,
}

impl MenuConfigService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn list(&self) -> AppResult<MenuConfigListResponse> {
        let sql = "SELECT id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at, published_at FROM menu_configs WHERE deleted_at IS NULL ORDER BY version DESC";
        let rows: Vec<MenuConfigRow> = match self.state.db.as_ref() {
            DbPool::Mysql(pool) => sqlx::query_as(sql).fetch_all(pool).await?,
            DbPool::Sqlite(pool) => sqlx::query_as(sql).fetch_all(pool).await?,
        };

        let total = rows.len() as u64;
        let items = rows.into_iter().map(Self::to_summary).collect();

        Ok(MenuConfigListResponse { items, total })
    }

    pub async fn create(&self, req: CreateMenuConfigRequest) -> AppResult<MenuConfigDetail> {
        validate_routes_json(&req.routes_json).map_err(AppError::Validation)?;

        let id = nanoid!();
        let next_version = self.next_version().await?;
        let routes_json = serde_json::to_string(&req.routes_json)
            .map_err(|e| AppError::Validation(e.to_string()))?;

        let row = match self.state.db.as_ref() {
            DbPool::Mysql(pool) => {
                sqlx::query_as("INSERT INTO menu_configs (id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 'draft', false, ?, NOW(), NOW()) RETURNING id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at, published_at")
                    .bind(&id)
                    .bind(next_version)
                    .bind(&req.name)
                    .bind(&req.description)
                    .bind(&routes_json)
                    .bind(&req.created_by)
                    .fetch_one(pool)
                    .await?
            }
            DbPool::Sqlite(pool) => {
                sqlx::query_as("INSERT INTO menu_configs (id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 'draft', 0, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP) RETURNING id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at, published_at")
                    .bind(&id)
                    .bind(next_version)
                    .bind(&req.name)
                    .bind(&req.description)
                    .bind(&routes_json)
                    .bind(&req.created_by)
                    .fetch_one(pool)
                    .await?
            }
        };

        Self::to_detail(row)
    }

    pub async fn get(&self, id: &str) -> AppResult<MenuConfigDetail> {
        let row = self.get_row(id).await?;
        Self::to_detail(row)
    }

    pub async fn update(
        &self,
        id: &str,
        req: UpdateMenuConfigRequest,
    ) -> AppResult<MenuConfigDetail> {
        let existing = self.get_row(id).await?;
        if existing.status != "draft" {
            return Err(AppError::Conflict(
                "only draft menu configs can be updated".to_string(),
            ));
        }

        let routes_value = match req.routes_json {
            Some(value) => {
                validate_routes_json(&value).map_err(AppError::Validation)?;
                value
            }
            None => serde_json::from_str(&existing.routes_json)
                .map_err(|e| AppError::Validation(e.to_string()))?,
        };

        let name = req.name.unwrap_or(existing.name);
        let description = req.description.or(existing.description);
        let routes_json = serde_json::to_string(&routes_value)
            .map_err(|e| AppError::Validation(e.to_string()))?;

        let row = match self.state.db.as_ref() {
            DbPool::Mysql(pool) => {
                sqlx::query_as("UPDATE menu_configs SET name = ?, description = ?, routes_json = ?, updated_at = NOW() WHERE id = ? AND deleted_at IS NULL RETURNING id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at, published_at")
                    .bind(&name)
                    .bind(&description)
                    .bind(&routes_json)
                    .bind(id)
                    .fetch_one(pool)
                    .await?
            }
            DbPool::Sqlite(pool) => {
                sqlx::query_as("UPDATE menu_configs SET name = ?, description = ?, routes_json = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND deleted_at IS NULL RETURNING id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at, published_at")
                    .bind(&name)
                    .bind(&description)
                    .bind(&routes_json)
                    .bind(id)
                    .fetch_one(pool)
                    .await?
            }
        };

        Self::to_detail(row)
    }

    pub async fn publish(&self, id: &str) -> AppResult<MenuConfigDetail> {
        let existing = self.get_row(id).await?;
        let routes_value: Value = serde_json::from_str(&existing.routes_json)
            .map_err(|e| AppError::Validation(e.to_string()))?;
        validate_routes_json(&routes_value).map_err(AppError::Validation)?;

        let row = match self.state.db.as_ref() {
            DbPool::Mysql(pool) => {
                let mut tx = pool.begin().await?;
                sqlx::query("UPDATE menu_configs SET is_active = false WHERE deleted_at IS NULL")
                    .execute(&mut *tx)
                    .await?;
                let row = sqlx::query_as("UPDATE menu_configs SET status = 'published', is_active = true, published_at = NOW(), updated_at = NOW() WHERE id = ? AND deleted_at IS NULL RETURNING id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at, published_at")
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await?;
                tx.commit().await?;
                row
            }
            DbPool::Sqlite(pool) => {
                let mut tx = pool.begin().await?;
                sqlx::query("UPDATE menu_configs SET is_active = 0 WHERE deleted_at IS NULL")
                    .execute(&mut *tx)
                    .await?;
                let row = sqlx::query_as("UPDATE menu_configs SET status = 'published', is_active = 1, published_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND deleted_at IS NULL RETURNING id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at, published_at")
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await?;
                tx.commit().await?;
                row
            }
        };

        Self::to_detail(row)
    }

    pub async fn clone_config(&self, id: &str) -> AppResult<MenuConfigDetail> {
        let existing = self.get_row(id).await?;
        let new_id = nanoid!();
        let next_version = self.next_version().await?;
        let cloned_name = format!("{} copy", existing.name);

        let row = match self.state.db.as_ref() {
            DbPool::Mysql(pool) => {
                sqlx::query_as("INSERT INTO menu_configs (id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 'draft', false, ?, NOW(), NOW()) RETURNING id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at, published_at")
                    .bind(&new_id)
                    .bind(next_version)
                    .bind(&cloned_name)
                    .bind(&existing.description)
                    .bind(&existing.routes_json)
                    .bind(&existing.created_by)
                    .fetch_one(pool)
                    .await?
            }
            DbPool::Sqlite(pool) => {
                sqlx::query_as("INSERT INTO menu_configs (id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 'draft', 0, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP) RETURNING id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at, published_at")
                    .bind(&new_id)
                    .bind(next_version)
                    .bind(&cloned_name)
                    .bind(&existing.description)
                    .bind(&existing.routes_json)
                    .bind(&existing.created_by)
                    .fetch_one(pool)
                    .await?
            }
        };

        Self::to_detail(row)
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let existing = self.get_row(id).await?;
        if existing.status == "published" || existing.is_active {
            return Err(AppError::Conflict(
                "published menu configs cannot be deleted".to_string(),
            ));
        }

        let rows_affected = match self.state.db.as_ref() {
            DbPool::Mysql(pool) => {
                sqlx::query("UPDATE menu_configs SET deleted_at = NOW(), updated_at = NOW() WHERE id = ? AND deleted_at IS NULL")
                    .bind(id)
                    .execute(pool)
                    .await?
                    .rows_affected()
            }
            DbPool::Sqlite(pool) => {
                sqlx::query("UPDATE menu_configs SET deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND deleted_at IS NULL")
                    .bind(id)
                    .execute(pool)
                    .await?
                    .rows_affected()
            }
        };

        if rows_affected == 0 {
            return Err(AppError::NotFound);
        }

        Ok(())
    }

    pub async fn active_routes(&self) -> AppResult<Value> {
        let sql = "SELECT routes_json FROM menu_configs WHERE is_active = true AND deleted_at IS NULL ORDER BY version DESC LIMIT 1";
        let row: Option<(String,)> = match self.state.db.as_ref() {
            DbPool::Mysql(pool) => sqlx::query_as(sql).fetch_optional(pool).await?,
            DbPool::Sqlite(pool) => sqlx::query_as(sql).fetch_optional(pool).await?,
        };

        match row {
            Some((routes_json,)) => {
                serde_json::from_str(&routes_json).map_err(|e| AppError::Validation(e.to_string()))
            }
            None => Ok(Value::Array(vec![])),
        }
    }

    async fn next_version(&self) -> AppResult<i64> {
        let sql = "SELECT COALESCE(MAX(version), 0) FROM menu_configs WHERE deleted_at IS NULL";
        let current: (i64,) = match self.state.db.as_ref() {
            DbPool::Mysql(pool) => sqlx::query_as(sql).fetch_one(pool).await?,
            DbPool::Sqlite(pool) => sqlx::query_as(sql).fetch_one(pool).await?,
        };

        Ok(current.0 + 1)
    }

    async fn get_row(&self, id: &str) -> AppResult<MenuConfigRow> {
        let sql = "SELECT id, version, name, description, routes_json, status, is_active, created_by, created_at, updated_at, published_at FROM menu_configs WHERE id = ? AND deleted_at IS NULL";
        let row: Option<MenuConfigRow> = match self.state.db.as_ref() {
            DbPool::Mysql(pool) => sqlx::query_as(sql).bind(id).fetch_optional(pool).await?,
            DbPool::Sqlite(pool) => sqlx::query_as(sql).bind(id).fetch_optional(pool).await?,
        };

        row.ok_or(AppError::NotFound)
    }

    fn to_summary(row: MenuConfigRow) -> MenuConfigSummary {
        MenuConfigSummary {
            id: row.id,
            version: row.version,
            name: row.name,
            description: row.description,
            status: row.status,
            is_active: row.is_active,
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
            published_at: row.published_at,
        }
    }

    fn to_detail(row: MenuConfigRow) -> AppResult<MenuConfigDetail> {
        let routes_json = serde_json::from_str(&row.routes_json)
            .map_err(|e| AppError::Validation(e.to_string()))?;

        Ok(MenuConfigDetail {
            id: row.id,
            version: row.version,
            name: row.name,
            description: row.description,
            routes_json,
            status: row.status,
            is_active: row.is_active,
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
            published_at: row.published_at,
        })
    }
}
