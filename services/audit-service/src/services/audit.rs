use crate::error::{AppError, AppResult};
use crate::models::{
    LoginLog, OperationLog, PagedLoginLogs, PagedOperationLogs, QueryLoginLogs, QueryOperationLogs,
};
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use digua_common_db::{db_fetch_all, db_fetch_optional, db_qb_fetch_all, db_qb_fetch_one, DbPool};

/// 审计日志查询服务。
///
/// 所有 SQL 只经 `digua_common_db` 的方言感知宏进出，本文件**不得**直接调用
/// `sqlx::query*`——否则 PG 的 `$N` 占位符与 `u32` 编码问题会分裂成两套代码。
///
/// 两条硬约束（都踩过）：
/// 1. PG 未实现 `Encode<Postgres> for u32`：`LIMIT` / `OFFSET` 一律 `as i64`。
/// 2. [`sqlx::QueryBuilder::push`] **不会**改写占位符（只有 `push_bind` 会调
///    `format_placeholder`）：闭包里不允许出现 `?`，要么 `push_bind`，要么静态 SQL。
pub struct AuditService {
    pool: DbPool,
}

impl AuditService {
    pub fn new(pool: &DbPool) -> Self {
        Self { pool: pool.clone() }
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }

    /// 某个用户最近的 N 条操作日志（按时间倒序）。
    pub async fn get_user_operation_logs(
        &self,
        user_id: &str,
        limit: i64,
    ) -> AppResult<Vec<OperationLog>> {
        let logs: Vec<OperationLog> = db_fetch_all!(
            &self.pool,
            OperationLog,
            "SELECT * FROM audit_operation_logs \
             WHERE user_id = ? \
             ORDER BY created_at DESC \
             LIMIT ?",
            user_id,
            limit
        )?;

        Ok(logs)
    }

    /// 某个用户最近的 N 条登录日志（按时间倒序）。
    pub async fn get_user_login_logs(&self, user_id: &str, limit: i64) -> AppResult<Vec<LoginLog>> {
        let logs: Vec<LoginLog> = db_fetch_all!(
            &self.pool,
            LoginLog,
            "SELECT * FROM audit_login_logs \
             WHERE user_id = ? \
             ORDER BY created_at DESC \
             LIMIT ?",
            user_id,
            limit
        )?;

        Ok(logs)
    }

    /// 按主键取单条操作日志。
    pub async fn get_operation_log(&self, id: i64) -> AppResult<Option<OperationLog>> {
        let log: Option<OperationLog> = db_fetch_optional!(
            &self.pool,
            OperationLog,
            "SELECT * FROM audit_operation_logs WHERE id = ?",
            id
        )?;

        Ok(log)
    }

    /// 按主键取单条登录日志。
    pub async fn get_login_log(&self, id: i64) -> AppResult<Option<LoginLog>> {
        let log: Option<LoginLog> = db_fetch_optional!(
            &self.pool,
            LoginLog,
            "SELECT * FROM audit_login_logs WHERE id = ?",
            id
        )?;

        Ok(log)
    }

    /// 分页查询操作日志（条件过滤 + 时间区间）。
    ///
    /// ⚠️ 原实现是「动态拼串 + 条件 `.bind()`」，这里改写成 `db_qb_*!` 闭包体：
    /// - `push_bind` 由 sqlx 自动产出 `?` / `$N`，两种后端同一份源码；
    /// - **`if let` 必须包住整段 `push` + `push_bind`**。把 `Option` 直接 `push_bind`
    ///   会生成 `AND user_id = $1`（`$1` 为 NULL），在三值逻辑下永不匹配 →
    ///   静默返回空集，且不报任何错。
    pub async fn query_operation_logs(
        &self,
        query: QueryOperationLogs,
    ) -> AppResult<PagedOperationLogs> {
        let page = query.page.unwrap_or(1);
        let size = query.size.unwrap_or(20);
        let offset = ((page - 1) * size) as i64;

        let user_id_ref = query.user_id.as_deref();
        let action_ref = query.action.as_deref();
        let resource_ref = query.resource.as_deref();
        // 查询串里的时间是**字符串**。MySQL 会隐式把字符串转成 DATETIME 比较，
        // PG 不转（直接 bind String 会撞 42804 text vs timestamptz），
        // 因此在 Rust 侧统一解析成 DateTime<Utc>，两侧都以时间戳参数绑定。
        let start_time = parse_time_bound(query.start_time.as_deref(), "start_time")?;
        let end_time = parse_time_bound(query.end_time.as_deref(), "end_time")?;

        // 注：total 沿用历史语义（只按 user_id / action / resource 过滤，
        // 不含时间区间），此处不改变既有行为。
        let total = self
            .count_operation_logs(user_id_ref, action_ref, resource_ref)
            .await?;

        let logs: Vec<OperationLog> = db_qb_fetch_all!(&self.pool, OperationLog, |qb| {
            qb.push("SELECT * FROM audit_operation_logs WHERE 1 = 1");
            if let Some(v) = user_id_ref {
                qb.push(" AND user_id = ").push_bind(v);
            }
            if let Some(v) = action_ref {
                qb.push(" AND action = ").push_bind(v);
            }
            if let Some(v) = resource_ref {
                qb.push(" AND resource = ").push_bind(v);
            }
            if let Some(v) = start_time {
                qb.push(" AND created_at >= ").push_bind(v);
            }
            if let Some(v) = end_time {
                qb.push(" AND created_at <= ").push_bind(v);
            }
            qb.push(" ORDER BY created_at DESC LIMIT ")
                .push_bind(size as i64);
            qb.push(" OFFSET ").push_bind(offset);
        })?;

        Ok(PagedOperationLogs {
            items: logs,
            total,
            page,
            size,
        })
    }

    /// 分页查询登录日志（用户名 / 成功与否 / 时间区间）。
    pub async fn query_login_logs(&self, query: QueryLoginLogs) -> AppResult<PagedLoginLogs> {
        let page = query.page.unwrap_or(1);
        let size = query.size.unwrap_or(20);
        let offset = ((page - 1) * size) as i64;

        let username_ref = query.username.as_deref();
        let success = query.success;
        let start_time = parse_time_bound(query.start_time.as_deref(), "start_time")?;
        let end_time = parse_time_bound(query.end_time.as_deref(), "end_time")?;

        let total = self.count_login_logs(username_ref, success).await?;

        let logs: Vec<LoginLog> = db_qb_fetch_all!(&self.pool, LoginLog, |qb| {
            qb.push("SELECT * FROM audit_login_logs WHERE 1 = 1");
            if let Some(v) = username_ref {
                qb.push(" AND username = ").push_bind(v);
            }
            if let Some(v) = success {
                qb.push(" AND success = ").push_bind(v);
            }
            if let Some(v) = start_time {
                qb.push(" AND created_at >= ").push_bind(v);
            }
            if let Some(v) = end_time {
                qb.push(" AND created_at <= ").push_bind(v);
            }
            qb.push(" ORDER BY created_at DESC LIMIT ")
                .push_bind(size as i64);
            qb.push(" OFFSET ").push_bind(offset);
        })?;

        Ok(PagedLoginLogs {
            items: logs,
            total,
            page,
            size,
        })
    }

    /// 操作日志计数（`total` 字段用）。
    ///
    /// 与历史语义一致：只按 `user_id` / `action` / `resource` 过滤。
    async fn count_operation_logs(
        &self,
        user_id: Option<&str>,
        action: Option<&str>,
        resource: Option<&str>,
    ) -> AppResult<i64> {
        let count: (i64,) = db_qb_fetch_one!(&self.pool, (i64,), |qb| {
            qb.push("SELECT COUNT(*) FROM audit_operation_logs WHERE 1 = 1");
            if let Some(v) = user_id {
                qb.push(" AND user_id = ").push_bind(v);
            }
            if let Some(v) = action {
                qb.push(" AND action = ").push_bind(v);
            }
            if let Some(v) = resource {
                qb.push(" AND resource = ").push_bind(v);
            }
        })?;
        Ok(count.0)
    }

    /// 登录日志计数（`total` 字段用）。
    async fn count_login_logs(
        &self,
        username: Option<&str>,
        success: Option<bool>,
    ) -> AppResult<i64> {
        let count: (i64,) = db_qb_fetch_one!(&self.pool, (i64,), |qb| {
            qb.push("SELECT COUNT(*) FROM audit_login_logs WHERE 1 = 1");
            if let Some(v) = username {
                qb.push(" AND username = ").push_bind(v);
            }
            if let Some(v) = success {
                qb.push(" AND success = ").push_bind(v);
            }
        })?;
        Ok(count.0)
    }

    /// 扁平查询（给 `AppServices::get_operation_logs_by_filter` 用）。
    ///
    /// `_start_date` / `_end_date` 保持原有语义：**当前不参与过滤**（历史行为，
    /// 未在本次方言改造中改动——加入过滤会改变既有查询结果）。
    #[allow(clippy::too_many_arguments)] // Public query API: callers pass filters + pagination directly.
    pub async fn query_logs(
        &self,
        user_id: Option<&str>,
        action: Option<&str>,
        resource: Option<&str>,
        _start_date: Option<DateTime<Utc>>,
        _end_date: Option<DateTime<Utc>>,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<OperationLog>> {
        let logs: Vec<OperationLog> = db_qb_fetch_all!(&self.pool, OperationLog, |qb| {
            qb.push("SELECT * FROM audit_operation_logs WHERE 1 = 1");
            if let Some(v) = user_id {
                qb.push(" AND user_id = ").push_bind(v);
            }
            if let Some(v) = action {
                qb.push(" AND action = ").push_bind(v);
            }
            if let Some(v) = resource {
                qb.push(" AND resource = ").push_bind(v);
            }
            qb.push(" ORDER BY created_at DESC LIMIT ").push_bind(limit);
            qb.push(" OFFSET ").push_bind(offset);
        })?;

        Ok(logs)
    }
}

/// 解析查询串里的时间边界（`start_time` / `end_time`）。
///
/// 为什么必须在 Rust 侧解析：MySQL 的 `DATETIME >= '2026-01-01'` 会隐式转换字符串，
/// PostgreSQL **不会**——直接 `bind(String)` 对上 `timestamptz` 列会报
/// `42804 column is of type timestamp with time zone but expression is of type text`。
/// 解析成 `DateTime<Utc>` 后，两种后端都以时间戳参数绑定，SQL 源码保持一致。
///
/// 接受格式：RFC3339（带时区）、`YYYY-MM-DD HH:MM[:SS[.fff]]`、
/// `YYYY-MM-DDTHH:MM[:SS[.fff]]`、纯日期 `YYYY-MM-DD`（按 UTC 零点）。
/// 空串视为「不过滤」（与查询串缺省一致）；无法解析返回 400——静默忽略会让
/// 过滤条件凭空消失，比报错危险得多。
fn parse_time_bound(raw: Option<&str>, field: &str) -> AppResult<Option<DateTime<Utc>>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let text = raw.trim();
    if text.is_empty() {
        return Ok(None);
    }

    if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
        return Ok(Some(dt.with_timezone(&Utc)));
    }
    for fmt in [
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M",
    ] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(text, fmt) {
            return Ok(Some(Utc.from_utc_datetime(&naive)));
        }
    }
    if let Ok(date) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        if let Some(naive) = date.and_hms_opt(0, 0, 0) {
            return Ok(Some(Utc.from_utc_datetime(&naive)));
        }
    }

    Err(AppError::bad_request(
        "AUD_LOG_1002",
        format!("invalid {field} (expect RFC3339 or YYYY-MM-DD[ HH:MM[:SS]]): {text}"),
    ))
}
