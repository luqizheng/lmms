//! Service JWT audience grants（WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01）。
//!
//! owner → audience 的签发授权关系：`POST /tokens/service-jwt` 在 bootstrap
//! token 校验通过后还要求存在一行 `status='active'` 的 grant，否则拒绝签发
//! （fail-closed：DB 异常按 500 处理，绝不放行）。
//!
//! 管理面（`routes/service_jwt_grant_{admin,web}.rs`）共用本模块的 CRUD +
//! audit。并发语义：
//!   - create 依赖 PK `(owner, audience)` 兜底——并发双插时第二个 INSERT 报
//!     唯一键冲突，ops 层映射 409；
//!   - set_status 用条件 UPDATE（`AND status <> ?`）防并发覆盖 / TOCTOU。
//!
//! 审计走同库 append-only 表 `token_service_jwt_grant_audit`（本仓库
//! `audit_events` 行形的 token-service 本地扩展；token-service 无 MQ/outbox
//! 依赖，跨服务审计链路是后续任务）。审计记录**永不**携带 token/JWT/凭据。

use crate::db::DbPool;
use digua_common_db::{db_execute, db_fetch_one, db_fetch_optional, db_qb_fetch_all, db_qb_fetch_one};

/// grant 的两种合法状态。
pub const STATUS_ACTIVE: &str = "active";
pub const STATUS_DISABLED: &str = "disabled";

/// 与 `bootstrap_token_admin::validate_owner` 一致的名称字符集：
/// 字母 / 数字 / `-` / `_`，1..=64。排除空白与 SQL 片段字符（参数化查询下
/// 仅剩防呆意义，但保持两套管理面语义一致）。
pub fn validate_name(value: &str) -> Result<(), &'static str> {
    if value.is_empty() || value.len() > 64 {
        return Err("length must be 1..=64");
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("may only contain [a-zA-Z0-9_-]");
    }
    Ok(())
}

/// 归一化 status 字符串到合法枚举；非法输入返回 None（调用方映射 400）。
pub fn parse_status(status: &str) -> Option<&'static str> {
    match status {
        STATUS_ACTIVE => Some(STATUS_ACTIVE),
        STATUS_DISABLED => Some(STATUS_DISABLED),
        _ => None,
    }
}

/// `token_service_jwt_grants` 行结构。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GrantRow {
    pub owner: String,
    pub audience: String,
    pub status: String,
    pub system_managed: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

const GRANT_COLUMNS: &str =
    "owner, audience, status, system_managed, created_at, updated_at";

// ─────────────────────────────────────────────────────────────────────────────
// 运行时检查（签发路径）
// ─────────────────────────────────────────────────────────────────────────────

/// 签发路径的授权判定：`(owner, audience)` 存在且 `status='active'` 才放行。
/// 行不存在 / disabled → `Ok(false)`；DB 错误上抛（调用方 500 fail-closed）。
pub async fn is_active_grant(
    pool: &DbPool,
    owner: &str,
    audience: &str,
) -> Result<bool, sqlx::Error> {
    let row: Option<(String,)> = db_fetch_optional!(
        pool,
        (String,),
        "SELECT status FROM token_service_jwt_grants \
         WHERE owner = ? AND audience = ?",
        owner,
        audience
    )?;
    Ok(matches!(row, Some((s,)) if s == STATUS_ACTIVE))
}

// ─────────────────────────────────────────────────────────────────────────────
// 管理面 CRUD（ops 层专用；鉴权在 routes 层）
// ─────────────────────────────────────────────────────────────────────────────

/// 分页 + 过滤查询。`page` 从 1 起；稳定排序 `owner, audience`。
/// 调用方（ops）已保证 page/size 边界合法。
pub async fn list_filtered(
    pool: &DbPool,
    owner: Option<&str>,
    audience: Option<&str>,
    status: Option<&str>,
    page: u32,
    size: u32,
) -> Result<(Vec<GrantRow>, i64), sqlx::Error> {
    let offset: i64 = ((page - 1) as i64) * (size as i64);
    let owner = owner.map(str::to_string);
    let audience = audience.map(str::to_string);
    let status = status.map(str::to_string);

    let rows: Vec<GrantRow> = db_qb_fetch_all!(pool, GrantRow, |qb| {
        qb.push("SELECT ")
            .push(GRANT_COLUMNS)
            .push(" FROM token_service_jwt_grants WHERE 1=1");
        if owner.is_some() {
            qb.push(" AND owner = ").push_bind(owner.as_ref().unwrap());
        }
        if audience.is_some() {
            qb.push(" AND audience = ")
                .push_bind(audience.as_ref().unwrap());
        }
        if status.is_some() {
            qb.push(" AND status = ").push_bind(status.as_ref().unwrap());
        }
        qb.push(" ORDER BY owner, audience LIMIT ")
            .push_bind(size as i64);
        qb.push(" OFFSET ").push_bind(offset);
    })?;

    let total: (i64,) = db_qb_fetch_one!(pool, (i64,), |qb| {
        qb.push("SELECT COUNT(*) FROM token_service_jwt_grants WHERE 1=1");
        if owner.is_some() {
            qb.push(" AND owner = ").push_bind(owner.as_ref().unwrap());
        }
        if audience.is_some() {
            qb.push(" AND audience = ")
                .push_bind(audience.as_ref().unwrap());
        }
        if status.is_some() {
            qb.push(" AND status = ").push_bind(status.as_ref().unwrap());
        }
    })?;

    Ok((rows, total.0))
}

/// 按 `(owner, audience)` 取单行。
pub async fn find(
    pool: &DbPool,
    owner: &str,
    audience: &str,
) -> Result<Option<GrantRow>, sqlx::Error> {
    let row: Option<GrantRow> = db_fetch_optional!(
        pool,
        GrantRow,
        "SELECT owner, audience, status, system_managed, created_at, updated_at \
         FROM token_service_jwt_grants WHERE owner = ? AND audience = ?",
        owner,
        audience
    )?;
    Ok(row)
}

/// create 的结果：`Inserted` 成功；`Duplicate` 已存在（含并发 PK 冲突）。
#[derive(Debug, PartialEq, Eq)]
pub enum CreateOutcome {
    Inserted,
    Duplicate,
}

/// 新建 grant（`status='active'`, `system_managed=false`）。
///
/// 先查后插 + PK 唯一约束兜底：并发创建相同 `(owner, audience)` 时恰一次
/// 返回 `Inserted`，另一边 INSERT 报唯一键冲突映射为 `Duplicate`。
pub async fn create(
    pool: &DbPool,
    owner: &str,
    audience: &str,
) -> Result<CreateOutcome, sqlx::Error> {
    if find(pool, owner, audience).await?.is_some() {
        return Ok(CreateOutcome::Duplicate);
    }
    let res = db_execute!(
        pool,
        "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
         VALUES (?, ?, 'active', FALSE)",
        owner,
        audience
    );
    match res {
        Ok(_) => Ok(CreateOutcome::Inserted),
        Err(e) => {
            if is_unique_violation(&e) {
                Ok(CreateOutcome::Duplicate)
            } else {
                Err(e)
            }
        }
    }
}

/// sqlx 唯一键冲突判定（MySQL 1062 / PG 23505）。
fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.is_unique_violation())
}

/// 启用 / 禁用。条件 UPDATE（`AND status <> ?`）保证并发下后写者不会把
/// 已被他人改到目标状态的行"再次变更"——重复请求返回 `affected=0`。
/// 返回受影响行数（0 = 行不存在或已处于目标状态）。
pub async fn set_status(
    pool: &DbPool,
    owner: &str,
    audience: &str,
    status: &str,
) -> Result<u64, sqlx::Error> {
    let affected = db_execute!(
        pool,
        "UPDATE token_service_jwt_grants SET status = ?, updated_at = ? \
         WHERE owner = ? AND audience = ? AND status <> ?",
        status,
        chrono::Utc::now(),
        owner,
        audience,
        status
    )?;
    Ok(affected)
}

/// 删除 grant。返回受影响行数（0 = 不存在；已删再删同为 0）。
/// `system_managed` 保护在 ops 层先查后删（需要 409 语义而非静默跳过）。
pub async fn delete(pool: &DbPool, owner: &str, audience: &str) -> Result<u64, sqlx::Error> {
    let affected = db_execute!(
        pool,
        "DELETE FROM token_service_jwt_grants WHERE owner = ? AND audience = ?",
        owner,
        audience
    )?;
    Ok(affected)
}

// ─────────────────────────────────────────────────────────────────────────────
// 系统授权初始化（seed）
// ─────────────────────────────────────────────────────────────────────────────

/// dev 默认 audience 全集：与 `bootstrap_token::DEFAULT_DEV_BOOTSTRAP` 的
/// owner 列表一致（7 个平台服务互相可签发，dev 栈开箱即用）。
pub const DEFAULT_DEV_AUDIENCES: &[&str] = &[
    "iam-service",
    "token-service",
    "notification-service",
    "audit-service",
    "prue_backend",
    "api-gateway",
    "file-service",
];

/// 解析 `SERVICE_JWT_GRANT_SEED`（`owner:audience` 逗号分隔）→ `(owner, audience)` 列表。
///
/// 空串 → 空列表；格式非法 → `Err`（启动拒绝，fail-closed）。
fn parse_seed_env(env: &str) -> Result<Vec<(String, String)>, String> {
    let mut pairs = Vec::new();
    for raw in env.split(',') {
        let item = raw.trim();
        if item.is_empty() {
            continue;
        }
        let (owner, audience) = item
            .split_once(':')
            .ok_or_else(|| format!("invalid grant seed item `{item}`: expected owner:audience"))?;
        let (owner, audience) = (owner.trim(), audience.trim());
        validate_name(owner).map_err(|e| format!("invalid grant seed owner `{owner}`: {e}"))?;
        validate_name(audience)
            .map_err(|e| format!("invalid grant seed audience `{audience}`: {e}"))?;
        pairs.push((owner.to_string(), audience.to_string()));
    }
    Ok(pairs)
}

/// 启动 seed（main.rs 在 bootstrap seed 之后调用）。
///
/// 两部分，均幂等（已有行不动）：
/// 1. grants 表为空 → 为每个 dev owner × [`DEFAULT_DEV_AUDIENCES`] 插
///    `active + system_managed=true` 行（内置系统授权，管理面不可改删）；
/// 2. env `SERVICE_JWT_GRANT_SEED`（`owner:audience,...`）→ 补插
///    `system_managed=false` 行（生产声明式初始化；格式非法 → `Err` 拒绝启动）。
///
/// 返回实际插入行数。
pub async fn seed_system_grants(pool: &DbPool, grant_seed_env: &str) -> Result<u64, sqlx::Error> {
    let env_pairs = parse_seed_env(grant_seed_env).map_err(|e| {
        sqlx::Error::Protocol(format!("SERVICE_JWT_GRANT_SEED invalid: {e}"))
    })?;

    let mut inserted = 0u64;

    // 1. 内置系统授权：仅当表完全为空（首启）时灌入，绝不覆盖已有数据。
    let total: (i64,) = db_fetch_one!(
        pool,
        (i64,),
        "SELECT COUNT(*) FROM token_service_jwt_grants"
    )?;
    if total.0 == 0 {
        for owner in crate::models::bootstrap_token::DEFAULT_DEV_BOOTSTRAP.iter().map(|(o, _)| *o) {
            for audience in DEFAULT_DEV_AUDIENCES {
                db_execute!(
                    pool,
                    "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
                     VALUES (?, ?, 'active', TRUE)",
                    owner,
                    audience
                )?;
                inserted += 1;
            }
        }
        tracing::info!("seeded built-in system grants ({} rows)", inserted);
    }

    // 2. env 声明式 seed：任何时候补插（幂等），普通（非保护）行。
    for (owner, audience) in &env_pairs {
        let existing: i64 = db_fetch_scalar_count(pool, owner, audience).await?;
        if existing > 0 {
            continue;
        }
        db_execute!(
            pool,
            "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
             VALUES (?, ?, 'active', FALSE)",
            owner,
            audience
        )?;
        inserted += 1;
    }

    Ok(inserted)
}

/// env seed 的存在性检查（复用 `find` 但只取标量，避免整行反序列化）。
async fn db_fetch_scalar_count(
    pool: &DbPool,
    owner: &str,
    audience: &str,
) -> Result<i64, sqlx::Error> {
    let row: Option<(i64,)> = db_fetch_optional!(
        pool,
        (i64,),
        "SELECT COUNT(*) FROM token_service_jwt_grants WHERE owner = ? AND audience = ?",
        owner,
        audience
    )?;
    Ok(row.map(|(c,)| c).unwrap_or(0))
}

// ─────────────────────────────────────────────────────────────────────────────
// 审计（append-only 本地表）
// ─────────────────────────────────────────────────────────────────────────────

/// 操作者身份（审计用）。service 入口来自 service-jwt 的 `sub`，web 入口来自
/// user-jwt 的 `user_id`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Actor {
    Service { subject: String },
    User { user_id: String },
}

impl Actor {
    pub fn actor_type(&self) -> &'static str {
        match self {
            Actor::Service { .. } => "service",
            Actor::User { .. } => "user",
        }
    }
    pub fn actor_id(&self) -> &str {
        match self {
            Actor::Service { subject } => subject,
            Actor::User { user_id } => user_id,
        }
    }
}

/// 审计动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    Create,
    /// 仅用于权限拒绝（denied）场景：尝试状态变更但被 403 挡下，
    /// 实际目标（enable/disable）未知。
    Update,
    Enable,
    Disable,
    Delete,
}

impl AuditAction {
    pub fn as_str(self) -> &'static str {
        match self {
            AuditAction::Create => "create",
            AuditAction::Update => "update",
            AuditAction::Enable => "enable",
            AuditAction::Disable => "disable",
            AuditAction::Delete => "delete",
        }
    }
}

/// 写一行审计。append-only：无 UPDATE / DELETE 路径。
///
/// `before`/`after` 为变更前后状态（`None` = 无行 / 不适用）；`result` 取
/// `success` / `denied`（系统保护或权限拒绝）；`detail` 为简短机器可读原因，
/// **不得**包含 token / JWT / 凭据 / SQL。
pub async fn record_audit(
    pool: &DbPool,
    actor: &Actor,
    owner: &str,
    audience: &str,
    action: AuditAction,
    before: Option<&str>,
    after: Option<&str>,
    request_id: &str,
    result: &str,
    detail: &str,
) -> Result<(), sqlx::Error> {
    db_execute!(
        pool,
        "INSERT INTO token_service_jwt_grant_audit \
         (actor_id, actor_type, owner, audience, action, before_status, after_status, \
          request_id, result, detail) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        actor.actor_id(),
        actor.actor_type(),
        owner,
        audience,
        action.as_str(),
        before,
        after,
        request_id,
        result,
        detail
    )?;
    Ok(())
}

/// 按时间倒序读审计（管理面查询用；数量由调用方限制）。
pub async fn list_audit(
    pool: &DbPool,
    owner: Option<&str>,
    audience: Option<&str>,
    limit: u32,
) -> Result<Vec<GrantAuditRow>, sqlx::Error> {
    let owner = owner.map(str::to_string);
    let audience = audience.map(str::to_string);
    let rows: Vec<GrantAuditRow> = db_qb_fetch_all!(pool, GrantAuditRow, |qb| {
        qb.push("SELECT id, actor_id, actor_type, owner, audience, action, before_status, \
                 after_status, request_id, result, detail, occurred_at \
                 FROM token_service_jwt_grant_audit WHERE 1=1");
        if owner.is_some() {
            qb.push(" AND owner = ").push_bind(owner.as_ref().unwrap());
        }
        if audience.is_some() {
            qb.push(" AND audience = ")
                .push_bind(audience.as_ref().unwrap());
        }
        qb.push(" ORDER BY id DESC LIMIT ").push_bind(limit as i64);
    })?;
    Ok(rows)
}

/// `token_service_jwt_grant_audit` 行结构。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GrantAuditRow {
    pub id: i64,
    pub actor_id: String,
    pub actor_type: String,
    pub owner: String,
    pub audience: String,
    pub action: String,
    pub before_status: Option<String>,
    pub after_status: Option<String>,
    pub request_id: String,
    pub result: String,
    pub detail: String,
    pub occurred_at: chrono::DateTime<chrono::Utc>,
}

// ─────────────────────────────────────────────────────────────────────────────
// 单元测试（纯函数：校验 / 状态 / seed 解析）
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_name_accepts_service_names() {
        assert!(validate_name("iam-service").is_ok());
        assert!(validate_name("prue_backend").is_ok());
        assert!(validate_name("a").is_ok());
        assert!(validate_name(&"x".repeat(64)).is_ok());
    }

    #[test]
    fn validate_name_rejects_empty_too_long_and_bad_chars() {
        assert!(validate_name("").is_err());
        assert!(validate_name(&"x".repeat(65)).is_err());
        assert!(validate_name("a b").is_err());
        assert!(validate_name("a;b").is_err());
        assert!(validate_name("a'b").is_err());
        assert!(validate_name("服务").is_err());
        assert!(validate_name("a#old").is_err());
    }

    #[test]
    fn parse_status_only_accepts_enums() {
        assert_eq!(parse_status("active"), Some(STATUS_ACTIVE));
        assert_eq!(parse_status("disabled"), Some(STATUS_DISABLED));
        assert_eq!(parse_status("ACTIVE"), None);
        assert_eq!(parse_status("enabled"), None);
        assert_eq!(parse_status(""), None);
        assert_eq!(parse_status("'; DROP TABLE x"), None);
    }

    #[test]
    fn parse_seed_env_accepts_valid_pairs() {
        let pairs = parse_seed_env("svc-a:iam-service, svc-b:file-service").expect("valid");
        assert_eq!(
            pairs,
            vec![
                ("svc-a".to_string(), "iam-service".to_string()),
                ("svc-b".to_string(), "file-service".to_string()),
            ]
        );
    }

    #[test]
    fn parse_seed_env_empty_is_empty() {
        assert!(parse_seed_env("").expect("empty").is_empty());
        assert!(parse_seed_env(" , ").expect("blank").is_empty());
    }

    #[test]
    fn parse_seed_env_rejects_missing_colon() {
        assert!(parse_seed_env("svc-a").is_err());
    }

    #[test]
    fn parse_seed_env_rejects_bad_charset() {
        assert!(parse_seed_env("svc a:iam").is_err());
        assert!(parse_seed_env("svc:iam service").is_err());
        assert!(parse_seed_env(":iam").is_err());
        assert!(parse_seed_env("svc:").is_err());
    }

    #[test]
    fn actor_type_and_id_roundtrip() {
        let s = Actor::Service {
            subject: "iam-service".into(),
        };
        let u = Actor::User {
            user_id: "usr-1".into(),
        };
        assert_eq!(s.actor_type(), "service");
        assert_eq!(s.actor_id(), "iam-service");
        assert_eq!(u.actor_type(), "user");
        assert_eq!(u.actor_id(), "usr-1");
    }
}
