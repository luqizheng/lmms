//! 菜单配置服务（prue_backend）。
//!
//! # 三方言（MySQL / PostgreSQL / SQLite）单份 SQL
//!
//! 改造前每个方法都是「两份 SQL + 两个 `DbPool` match 臂」（共 9 对，18 臂）。
//! 加 PostgreSQL 就变成三份，正是设计文档 §2.1.3 禁止的形态。现在收敛成：
//!
//! - 方言差异**只**出现在三个纯文本 helper 里（字段列表 / json→text 表达式 / 当前时间表达式）；
//! - 其余全部是**一份** SQL，执行走 `db_*!` 宏（宏负责 `?` → `$N` 的占位符改写）。
//!
//! 唯一保留的 `match` 是 [`MenuConfigService::publish`] 的事务分支：
//! `sqlx::Transaction<'_, DB>` 的类型参数随后端变化，基座层（`digua_common_db`）
//! 目前没有跨方言事务抽象，因此「先全量置 false 再激活目标」这两条语句的原子性
//! 必须按后端分别 `begin/commit`。详见该方法的注释。

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
use digua_common_db::dialect::DialectKind;
use digua_common_db::{db_execute, db_fetch_all, db_fetch_optional, db_fetch_scalar, DbPool};
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

/// json 列取成字符串的方言表达式。
///
/// - MySQL：`JSON` 列不能直接解到 `String`，必须 `CAST(... AS CHAR)`；
/// - PostgreSQL：json 类型（OID `json`）sqlx 没有 `String` 的解码实现，
///   且 `sqlx::query_as` 不做隐式转换，必须显式 `::text`；
/// - SQLite：json 存的就是 TEXT，原样取。
fn routes_json_text_expr(dialect: DialectKind) -> &'static str {
    match dialect {
        DialectKind::Postgres => "routes_json::text",
        DialectKind::Mysql => "CAST(routes_json AS CHAR)",
        _ => "routes_json",
    }
}

/// `MenuConfigRow` 的 SELECT 字段列表（唯一的字段来源，避免三份重复）。
fn select_fields(dialect: DialectKind) -> String {
    format!(
        "id, version, name, description, {} AS routes_json, routes_etag, status, is_active, \
         created_by, created_at, updated_at, published_at",
        routes_json_text_expr(dialect)
    )
}

/// 「当前时间」的方言表达式。
///
/// MySQL 沿用历史的 `NOW()`（本地时区，与既有 `TIMESTAMP` 列一致，不做行为变更）；
/// PostgreSQL 用 `now()`（`TIMESTAMPTZ`，会话时区已由 `init_pool_with` 锁 UTC）；
/// SQLite 用 `CURRENT_TIMESTAMP`。
fn now_expr(dialect: DialectKind) -> &'static str {
    match dialect {
        DialectKind::Postgres => "now()",
        DialectKind::Sqlite => "CURRENT_TIMESTAMP",
        _ => "NOW()",
    }
}

/// 写入 json 列时占位符后面的强转后缀。
///
/// PostgreSQL 的 `json` 列**不接受 TEXT 参数**：sqlx 以 TEXT 类型 OID 发送 `String`，
/// 而 `text → json` 在 pg_cast 里没有 assignment 转换，服务端直接报
/// `42804 column "routes_json" is of type json but expression is of type text`，
/// 必须显式 `$n::json`。MySQL 的 `JSON` 列接受字符串、SQLite 的 json 就是 TEXT，
/// 两者都不需要强转。
fn json_param_cast(dialect: DialectKind) -> &'static str {
    match dialect {
        DialectKind::Postgres => "::json",
        _ => "",
    }
}

pub struct MenuConfigService {
    state: AppState,
}

impl MenuConfigService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn list(&self) -> AppResult<MenuConfigListResponse> {
        let sql = format!(
            "SELECT {} FROM prue_menu_configs WHERE deleted_at IS NULL ORDER BY version DESC",
            select_fields(self.state.db.dialect())
        );
        let rows: Vec<MenuConfigRow> = db_fetch_all!(self.state.db.as_ref(), MenuConfigRow, sql)?;

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

        // `is_active` 绑定 bool（而不是方言字面量 `false` / `0`）：
        // MySQL TINYINT(1) / PostgreSQL BOOLEAN / SQLite INTEGER 三种列都能接受。
        // `routes_json` 后面拼方言强转后缀（PG 的 json 列必须显式 `::json`）。
        let dialect = self.state.db.dialect();
        let sql = format!(
            "INSERT INTO prue_menu_configs \
             (id, version, name, description, routes_json, routes_etag, status, is_active, \
              created_by, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?{json_cast}, ?, 'draft', ?, ?, {now}, {now})",
            json_cast = json_param_cast(dialect),
            now = now_expr(dialect)
        );
        db_execute!(
            self.state.db.as_ref(),
            sql,
            &id,
            next_version,
            &req.name,
            &req.description,
            &routes_json,
            &routes_etag,
            false,
            &req.created_by
        )?;

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
            .ok_or_else(|| {
                AppError::Validation("if_match_etag is required for updates".to_string())
            })?;

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

        let dialect = self.state.db.dialect();
        let sql = format!(
            "UPDATE prue_menu_configs SET name = ?, description = ?, routes_json = ?{json_cast}, \
             routes_etag = ?, updated_at = {now} \
             WHERE id = ? AND deleted_at IS NULL AND routes_etag = ?",
            json_cast = json_param_cast(dialect),
            now = now_expr(dialect)
        );
        db_execute!(
            self.state.db.as_ref(),
            sql,
            &name,
            &description,
            &routes_json,
            &new_etag,
            id,
            &existing.routes_etag
        )?;

        self.get(id).await
    }

    pub async fn publish(&self, id: &str) -> AppResult<MenuConfigDetail> {
        let existing = self.get_row(id).await?;
        let routes_value: Value = serde_json::from_str(&existing.routes_json)
            .map_err(|e| AppError::Validation(e.to_string()))?;
        validate_routes_json(&routes_value).map_err(AppError::Validation)?;

        // ⚠️ 此处是**唯一保留**的 `DbPool` match，原因是基座层没有跨方言事务抽象：
        // `sqlx::Transaction<'_, DB>` 的 `DB` 是具体类型（MySql / Postgres / Sqlite），
        // 无法用同一个值承载三种后端，因此 `begin()` 必须按后端分别取。
        // 「全量置 false」+「激活目标」两条语句必须原子生效（否则并发发布会短暂没有激活项），
        // 所以不能简单拆成两次 `db_execute!` 而丢掉事务。
        // MySQL 与 PostgreSQL 的 SQL 文本只差 `NOW()` / `now()` 与占位符写法。
        match self.state.db.as_ref() {
            DbPool::Mysql(pool) => {
                let mut tx = pool.begin().await?;
                sqlx::query(
                    "UPDATE prue_menu_configs SET is_active = false WHERE deleted_at IS NULL",
                )
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE prue_menu_configs SET status = 'published', is_active = true, \
                     published_at = NOW(), updated_at = NOW() \
                     WHERE id = ? AND deleted_at IS NULL",
                )
                .bind(id)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
            }
            // 该臂随 feature 存在性裁剪：未开 `postgres` 时 `DbPool` 没有 Postgres 变体。
            #[cfg(feature = "postgres")]
            DbPool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                sqlx::query(
                    "UPDATE prue_menu_configs SET is_active = false WHERE deleted_at IS NULL",
                )
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE prue_menu_configs SET status = 'published', is_active = true, \
                     published_at = now(), updated_at = now() \
                     WHERE id = $1 AND deleted_at IS NULL",
                )
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
                sqlx::query(
                    "UPDATE prue_menu_configs SET status = 'published', is_active = 1, \
                     published_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
                     WHERE id = ? AND deleted_at IS NULL",
                )
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

        let dialect = self.state.db.dialect();
        let sql = format!(
            "INSERT INTO prue_menu_configs \
             (id, version, name, description, routes_json, routes_etag, status, is_active, \
              created_by, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?{json_cast}, ?, 'draft', ?, ?, {now}, {now})",
            json_cast = json_param_cast(dialect),
            now = now_expr(dialect)
        );
        db_execute!(
            self.state.db.as_ref(),
            sql,
            &new_id,
            next_version,
            &cloned_name,
            &existing.description,
            &existing.routes_json,
            &existing.routes_etag,
            false,
            &existing.created_by
        )?;

        self.get(&new_id).await
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let existing = self.get_row(id).await?;
        if existing.status == "published" || existing.is_active {
            return Err(AppError::Conflict(
                "published menu configs cannot be deleted".to_string(),
            ));
        }

        let now = now_expr(self.state.db.dialect());
        let sql = format!(
            "UPDATE prue_menu_configs SET deleted_at = {now}, updated_at = {now} \
             WHERE id = ? AND deleted_at IS NULL"
        );
        let rows_affected = db_execute!(self.state.db.as_ref(), sql, id)?;

        if rows_affected == 0 {
            return Err(AppError::NotFound);
        }

        Ok(())
    }

    pub async fn active_routes(&self) -> AppResult<Value> {
        // 只选一个字段，因此不复用 `select_fields`，但复用 json→text 表达式。
        let sql = format!(
            "SELECT {} AS routes_json FROM prue_menu_configs \
             WHERE is_active = TRUE AND deleted_at IS NULL ORDER BY version DESC LIMIT 1",
            routes_json_text_expr(self.state.db.dialect())
        );
        let row: Option<(String,)> = db_fetch_optional!(self.state.db.as_ref(), (String,), sql)?;

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
        // Includes soft-deleted rows: version never reuses a number, otherwise
        // rewind-after-delete collides with UNIQUE(version).
        // 三方言同一份 SQL：`COALESCE(MAX(version), 0)`。
        let sql = "SELECT COALESCE(MAX(version), 0) FROM prue_menu_configs";
        let current: i64 = db_fetch_scalar!(self.state.db.as_ref(), i64, sql)?;

        Ok(current + 1)
    }

    async fn get_row(&self, id: &str) -> AppResult<MenuConfigRow> {
        let sql = format!(
            "SELECT {} FROM prue_menu_configs WHERE id = ? AND deleted_at IS NULL",
            select_fields(self.state.db.dialect())
        );
        let row: Option<MenuConfigRow> =
            db_fetch_optional!(self.state.db.as_ref(), MenuConfigRow, sql, id)?;

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
