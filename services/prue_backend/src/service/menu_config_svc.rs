use crate::{
    dto::{
        validate_routes_json, CreateMenuConfigRequest, MenuConfigDetail, MenuConfigListResponse,
        MenuConfigSummary, UpdateMenuConfigRequest,
    },
    error::{AppError, AppResult},
    service::{etag, sanitize},
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
    routes_etag: String,
    status: String,
    is_active: bool,
    created_by: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    published_at: Option<DateTime<Utc>>,
}

const SELECT_FIELDS_MYSQL: &str = "id, version, name, description, CAST(routes_json AS CHAR) AS routes_json, routes_etag, status, is_active, created_by, created_at, updated_at, published_at";
const SELECT_FIELDS_SQLITE: &str = "id, version, name, description, routes_json, routes_etag, status, is_active, created_by, created_at, updated_at, published_at";

pub struct MenuConfigService {
    state: AppState,
}

impl MenuConfigService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn list(&self) -> AppResult<MenuConfigListResponse> {
        let mysql_sql = format!(
            "SELECT {} FROM prue_menu_configs WHERE deleted_at IS NULL ORDER BY version DESC",
            SELECT_FIELDS_MYSQL
        );
        let sqlite_sql = format!(
            "SELECT {} FROM prue_menu_configs WHERE deleted_at IS NULL ORDER BY version DESC",
            SELECT_FIELDS_SQLITE
        );
        let rows: Vec<MenuConfigRow> = match self.state.db.as_ref() {
            DbPool::Mysql(pool) => sqlx::query_as(&mysql_sql).fetch_all(pool).await?,
            DbPool::Sqlite(pool) => sqlx::query_as(&sqlite_sql).fetch_all(pool).await?,
        };

        let total = rows.len() as u64;
        let items = rows.into_iter().map(Self::to_summary).collect();

        Ok(MenuConfigListResponse { items, total })
    }

    pub async fn create(&self, req: CreateMenuConfigRequest) -> AppResult<MenuConfigDetail> {
        validate_routes_json(&req.routes_json).map_err(AppError::Validation)?;

        let id = nanoid!();
        let next_version = self.next_version().await?;
        let routes_etag = etag::compute(&req.routes_json);
        let routes_json = serde_json::to_string(&req.routes_json)
            .map_err(|e| AppError::Validation(e.to_string()))?;

        match self.state.db.as_ref() {
            DbPool::Mysql(pool) => {
                sqlx::query("INSERT INTO prue_menu_configs (id, version, name, description, routes_json, routes_etag, status, is_active, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 'draft', false, ?, NOW(), NOW())")
                    .bind(&id)
                    .bind(next_version)
                    .bind(&req.name)
                    .bind(&req.description)
                    .bind(&routes_json)
                    .bind(&routes_etag)
                    .bind(&req.created_by)
                    .execute(pool)
                    .await?;
            }
            DbPool::Sqlite(pool) => {
                sqlx::query("INSERT INTO prue_menu_configs (id, version, name, description, routes_json, routes_etag, status, is_active, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 'draft', 0, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
                    .bind(&id)
                    .bind(next_version)
                    .bind(&req.name)
                    .bind(&req.description)
                    .bind(&routes_json)
                    .bind(&routes_etag)
                    .bind(&req.created_by)
                    .execute(pool)
                    .await?;
            }
        }

        self.get(&id).await
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

        let if_match = req
            .if_match_etag
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AppError::Validation("if_match_etag is required for updates".to_string()))?;

        if if_match != existing.routes_etag {
            return Err(AppError::EtagConflict {
                current_etag: existing.routes_etag.clone(),
                updated_at: existing.updated_at,
            });
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
        let new_etag = etag::compute(&routes_value);
        let routes_json = serde_json::to_string(&routes_value)
            .map_err(|e| AppError::Validation(e.to_string()))?;

        match self.state.db.as_ref() {
            DbPool::Mysql(pool) => {
                sqlx::query("UPDATE prue_menu_configs SET name = ?, description = ?, routes_json = ?, routes_etag = ?, updated_at = NOW() WHERE id = ? AND deleted_at IS NULL AND routes_etag = ?")
                    .bind(&name)
                    .bind(&description)
                    .bind(&routes_json)
                    .bind(&new_etag)
                    .bind(id)
                    .bind(&existing.routes_etag)
                    .execute(pool)
                    .await?;
            }
            DbPool::Sqlite(pool) => {
                sqlx::query("UPDATE prue_menu_configs SET name = ?, description = ?, routes_json = ?, routes_etag = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND deleted_at IS NULL AND routes_etag = ?")
                    .bind(&name)
                    .bind(&description)
                    .bind(&routes_json)
                    .bind(&new_etag)
                    .bind(id)
                    .bind(&existing.routes_etag)
                    .execute(pool)
                    .await?;
            }
        }

        self.get(id).await
    }

    pub async fn publish(&self, id: &str) -> AppResult<MenuConfigDetail> {
        let existing = self.get_row(id).await?;
        let routes_value: Value = serde_json::from_str(&existing.routes_json)
            .map_err(|e| AppError::Validation(e.to_string()))?;
        validate_routes_json(&routes_value).map_err(AppError::Validation)?;

        match self.state.db.as_ref() {
            DbPool::Mysql(pool) => {
                let mut tx = pool.begin().await?;
                sqlx::query("UPDATE prue_menu_configs SET is_active = false WHERE deleted_at IS NULL")
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("UPDATE prue_menu_configs SET status = 'published', is_active = true, published_at = NOW(), updated_at = NOW() WHERE id = ? AND deleted_at IS NULL")
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
            }
            DbPool::Sqlite(pool) => {
                let mut tx = pool.begin().await?;
                sqlx::query("UPDATE prue_menu_configs SET is_active = 0 WHERE deleted_at IS NULL")
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("UPDATE prue_menu_configs SET status = 'published', is_active = 1, published_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND deleted_at IS NULL")
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
            }
        }

        self.get(id).await
    }

    pub async fn clone_config(&self, id: &str) -> AppResult<MenuConfigDetail> {
        let existing = self.get_row(id).await?;
        let new_id = nanoid!();
        let next_version = self.next_version().await?;
        let cloned_name = format!("{} copy", existing.name);

        match self.state.db.as_ref() {
            DbPool::Mysql(pool) => {
                sqlx::query("INSERT INTO prue_menu_configs (id, version, name, description, routes_json, routes_etag, status, is_active, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 'draft', false, ?, NOW(), NOW())")
                    .bind(&new_id)
                    .bind(next_version)
                    .bind(&cloned_name)
                    .bind(&existing.description)
                    .bind(&existing.routes_json)
                    .bind(&existing.routes_etag)
                    .bind(&existing.created_by)
                    .execute(pool)
                    .await?;
            }
            DbPool::Sqlite(pool) => {
                sqlx::query("INSERT INTO prue_menu_configs (id, version, name, description, routes_json, routes_etag, status, is_active, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 'draft', 0, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
                    .bind(&new_id)
                    .bind(next_version)
                    .bind(&cloned_name)
                    .bind(&existing.description)
                    .bind(&existing.routes_json)
                    .bind(&existing.routes_etag)
                    .bind(&existing.created_by)
                    .execute(pool)
                    .await?;
            }
        }

        self.get(&new_id).await
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
                sqlx::query("UPDATE prue_menu_configs SET deleted_at = NOW(), updated_at = NOW() WHERE id = ? AND deleted_at IS NULL")
                    .bind(id)
                    .execute(pool)
                    .await?
                    .rows_affected()
            }
            DbPool::Sqlite(pool) => {
                sqlx::query("UPDATE prue_menu_configs SET deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND deleted_at IS NULL")
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
        let mysql_sql = "SELECT CAST(routes_json AS CHAR) AS routes_json FROM prue_menu_configs WHERE is_active = true AND deleted_at IS NULL ORDER BY version DESC LIMIT 1";
        let sqlite_sql = "SELECT routes_json FROM prue_menu_configs WHERE is_active = true AND deleted_at IS NULL ORDER BY version DESC LIMIT 1";
        let row: Option<(String,)> = match self.state.db.as_ref() {
            DbPool::Mysql(pool) => sqlx::query_as(mysql_sql).fetch_optional(pool).await?,
            DbPool::Sqlite(pool) => sqlx::query_as(sqlite_sql).fetch_optional(pool).await?,
        };

        match row {
            Some((routes_json,)) => {
                let mut value: Value = serde_json::from_str(&routes_json)
                    .map_err(|e| AppError::Validation(e.to_string()))?;
                sanitize::strip_internal_fields(&mut value);
                Ok(value)
            }
            None => Ok(Value::Array(vec![])),
        }
    }

    async fn next_version(&self) -> AppResult<i64> {
        let sql = "SELECT COALESCE(MAX(version), 0) FROM prue_menu_configs WHERE deleted_at IS NULL";
        let current: (i64,) = match self.state.db.as_ref() {
            DbPool::Mysql(pool) => sqlx::query_as(sql).fetch_one(pool).await?,
            DbPool::Sqlite(pool) => sqlx::query_as(sql).fetch_one(pool).await?,
        };

        Ok(current.0 + 1)
    }

    async fn get_row(&self, id: &str) -> AppResult<MenuConfigRow> {
        let mysql_sql = format!(
            "SELECT {} FROM prue_menu_configs WHERE id = ? AND deleted_at IS NULL",
            SELECT_FIELDS_MYSQL
        );
        let sqlite_sql = format!(
            "SELECT {} FROM prue_menu_configs WHERE id = ? AND deleted_at IS NULL",
            SELECT_FIELDS_SQLITE
        );
        let row: Option<MenuConfigRow> = match self.state.db.as_ref() {
            DbPool::Mysql(pool) => sqlx::query_as(&mysql_sql).bind(id).fetch_optional(pool).await?,
            DbPool::Sqlite(pool) => sqlx::query_as(&sqlite_sql).bind(id).fetch_optional(pool).await?,
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
            routes_etag: row.routes_etag,
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
            routes_etag: row.routes_etag,
        })
    }
}
