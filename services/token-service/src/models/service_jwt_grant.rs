//! WP-LMMS-SERVICE-JWT-AUDIENCE-02: Service JWT owner→audience grants.
//!
//! 签发面授权：`POST /tokens/service-jwt` 在签名前必须确认
//! `(canonical owner, audience)` 存在 active grant。缺省一律拒绝
//! （fail-closed）——无通配符、无隐式 owner==audience。
//!
//! 内置系统授权（`SYSTEM_GRANTS`）是 code-owned、版本控制的默认值，
//! 由启动 seed（`ensure_system_grants`）幂等补插，必须在 HTTP listener
//! 接受请求之前完成。grant 属于 canonical owner（不含宽限期 `#old`
//! 后缀），与 Bootstrap Token 密钥值解耦——rotate 不影响授权。

use crate::db::{is_duplicate_key, DbPool};
use digua_common_db::{
    db_execute, db_fetch_all, db_fetch_optional, db_fetch_scalar, db_qb_fetch_all, db_qb_fetch_one,
    db_tx_execute, db_tx_fetch_optional, Transaction,
};

/// 内置系统授权（code-owned，版本控制）。
///
/// 只收录有具体调用点证据的 Bootstrap-Token 链路：
/// - `iam-service → token-service`：IAM 登录链。`token_forward.rs` 持
///   ServiceJwtCache 在 `POST /tokens/user-jwt` 前取 `aud=token-service`
///   凭证（P0，登录必需）。
/// - `notification-service → iam-service`：Notification 的 IAM 集成。
///   `iam_client.rs` 为 membership bootstrap 取 `aud=iam-service` 凭证
///   （P1，Notification 启用时）。
///
/// ⚠️ `token-service` 自身拥有 Bootstrap Token 身份，但其出站调用走
/// `mint_service_bearer_for_state` 自签路径（独立信任机制），**不**创建
/// `token-service → token-service` grant。
pub const SYSTEM_GRANTS: &[(&str, &str)] = &[
    ("iam-service", "token-service"),
    ("notification-service", "iam-service"),
];

/// P0 授权的 owner：缺少该 principal 的 active Bootstrap Token 时，
/// token-service 必须拒绝启动（登录链会死锁，静默降级不可接受）。
pub const REQUIRED_GRANT_PRINCIPAL: &str = "iam-service";

/// grant 表行结构。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ServiceJwtGrantRow {
    pub owner: String,
    pub audience: String,
    pub status: String,
    pub system_managed: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// 把 Bootstrap Token 行身份规范化为授权主体。
///
/// 宽限期遗留行（rotate 产生）的 owner 带 `#old` 后缀；grant 只按
/// canonical owner 存取——宽限 token 与正式 token 必须解析到同一主体、
/// 同一授权集合。
pub fn canonical_owner(owner: &str) -> &str {
    owner.strip_suffix("#old").unwrap_or(owner)
}

/// audience / owner 的合法字符集与长度（与 bootstrap admin 的
/// `validate_owner` 同规则）：1..=64，`[a-zA-Z0-9_-]`。
pub fn validate_service_identity(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 64 {
        return Err("service identity length must be 1..=64".to_string());
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("service identity may only contain [a-zA-Z0-9_-]".to_string());
    }
    Ok(())
}

/// 幂等补插内置系统 grants。
///
/// - 先查后插：已存在的行（含被人工 disable 的）一律不动——启动修复
///   只补缺失行，不悄悄复活被显式撤销的授权；
/// - 主键冲突吞掉：并发多副本同时 seed 时只有一个 INSERT 生效，其余
///   安全降级为幂等（fail-closed 由唯一约束兜底）；
/// - 返回实际插入行数。
pub async fn ensure_system_grants(pool: &DbPool) -> Result<u64, sqlx::Error> {
    let mut inserted = 0u64;
    for (owner, audience) in SYSTEM_GRANTS {
        let existing: i64 = db_fetch_scalar!(
            pool,
            i64,
            "SELECT COUNT(*) FROM token_service_jwt_grants WHERE owner = ? AND audience = ?",
            owner,
            audience
        )?;
        if existing > 0 {
            continue;
        }
        let res = db_execute!(
            pool,
            "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
             VALUES (?, ?, 'active', TRUE)",
            owner,
            audience
        );
        match res {
            Ok(_) => {
                inserted += 1;
                tracing::info!(
                    "seeded system service-jwt grant owner={} audience={}",
                    owner,
                    audience
                );
            }
            // 并发副本竞态：唯一约束判负的副本视为已完成。
            Err(ref e) if is_duplicate_key(e) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(inserted)
}

/// 签发前的强制授权检查。
///
/// **fail-closed 契约**：DB 错误向上传播（调用方 500，绝不签发）；
/// 无行 / 非 active → `false`（调用方 403）。
pub async fn is_allowed(pool: &DbPool, owner: &str, audience: &str) -> Result<bool, sqlx::Error> {
    let n: i64 = db_fetch_scalar!(
        pool,
        i64,
        "SELECT COUNT(*) FROM token_service_jwt_grants \
         WHERE owner = ? AND audience = ? AND status = 'active'",
        owner,
        audience
    )?;
    Ok(n > 0)
}

/// 按主体+audience 取单行（诊断 / 测试用）。
pub async fn find_by_owner_and_audience(
    pool: &DbPool,
    owner: &str,
    audience: &str,
) -> Result<Option<ServiceJwtGrantRow>, sqlx::Error> {
    let row: Option<ServiceJwtGrantRow> = db_fetch_optional!(
        pool,
        ServiceJwtGrantRow,
        "SELECT owner, audience, status, system_managed, created_at, updated_at \
         FROM token_service_jwt_grants WHERE owner = ? AND audience = ?",
        owner,
        audience
    )?;
    Ok(row)
}

/// 列出全部 grants（诊断 / 测试用）。
pub async fn list_all(pool: &DbPool) -> Result<Vec<ServiceJwtGrantRow>, sqlx::Error> {
    let rows: Vec<ServiceJwtGrantRow> = db_fetch_all!(
        pool,
        ServiceJwtGrantRow,
        "SELECT owner, audience, status, system_managed, created_at, updated_at \
         FROM token_service_jwt_grants ORDER BY owner, audience"
    )?;
    Ok(rows)
}

/// 启动时校验每个系统 grant 的 owner 都有 active Bootstrap Token。
///
/// 显式决策（WP-02 §3.6）：
/// - `iam-service`（P0 登录链）缺失 → **Err，token-service 拒绝启动**；
/// - `notification-service`（P1，可选部署）缺失 → 仅 warn；grant 行
///   仍会 seed，NTF 部署后即可用，无需人工干预。
///
/// 错误信息不含任何 secret 材料。
pub async fn validate_required_principals(pool: &DbPool) -> Result<(), String> {
    for (owner, audience) in SYSTEM_GRANTS {
        let active: i64 = db_fetch_scalar!(
            pool,
            i64,
            "SELECT COUNT(*) FROM token_bootstrap_tokens \
             WHERE owner = ? AND status = 'active' \
               AND (expires_at IS NULL OR expires_at > NOW())",
            owner
        )
        .map_err(|e| format!("principal check for '{owner}' failed: {e}"))?;
        if active > 0 {
            continue;
        }
        if *owner == REQUIRED_GRANT_PRINCIPAL {
            return Err(format!(
                "P0 service-jwt grant principal '{owner}' (audience '{audience}') \
                 has no active bootstrap token; token-service refuses to start. \
                 Configure SERVICE_BOOTSTRAP_TOKENS / SERVICE_BOOTSTRAP_OWNERS \
                 or seed the principal before startup."
            ));
        }
        tracing::warn!(
            "optional service-jwt grant principal '{}' (audience '{}') is not configured; \
             grant stays seeded and takes effect once the service is deployed",
            owner,
            audience
        );
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 管理面 CRUD + 审计（WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01）
//
// WP-02 只建立了运行时授权与 code-owned seed（"本阶段不提供 grant CRUD"）；
// 本段补齐授权管理控制面，供 routes/service_jwt_grant_{admin,web}.rs 共用。
// 并发语义：
//   - create 依赖 PK (owner, audience) 兜底——并发双插时第二个 INSERT 报
//     唯一键冲突，ops 层映射 409；
//   - set_status / delete 的条件写（`AND status <> ?` / `AND system_managed
//     = FALSE`）把并发去重与系统保护都下沉到写 SQL 内原子生效（R1-02），
//     ops 层对 `affected=0` 重查归类 404 / 409 / 幂等。
// 审计走同库 append-only 表 `token_service_jwt_grant_audit`（V006），记录
// 变更与拒绝，**永不**携带 token/JWT/凭据。
// ─────────────────────────────────────────────────────────────────────────────

/// grant 的两种合法状态。
pub const STATUS_ACTIVE: &str = "active";
pub const STATUS_DISABLED: &str = "disabled";

/// 归一化 status 字符串到合法枚举；非法输入返回 None（调用方映射 400）。
pub fn parse_status(status: &str) -> Option<&'static str> {
    match status {
        STATUS_ACTIVE => Some(STATUS_ACTIVE),
        STATUS_DISABLED => Some(STATUS_DISABLED),
        _ => None,
    }
}

/// 分页 + 过滤查询。`page` 从 1 起；稳定排序 `owner, audience`。
/// 调用方（ops）已保证 page/size 边界合法。
pub async fn list_filtered(
    pool: &DbPool,
    owner: Option<&str>,
    audience: Option<&str>,
    status: Option<&str>,
    page: u32,
    size: u32,
) -> Result<(Vec<ServiceJwtGrantRow>, i64), sqlx::Error> {
    let offset: i64 = ((page - 1) as i64) * (size as i64);
    let owner = owner.map(str::to_string);
    let audience = audience.map(str::to_string);
    let status = status.map(str::to_string);

    let rows: Vec<ServiceJwtGrantRow> = db_qb_fetch_all!(pool, ServiceJwtGrantRow, |qb| {
        qb.push(
            "SELECT owner, audience, status, system_managed, created_at, updated_at \
                 FROM token_service_jwt_grants WHERE 1=1",
        );
        if let Some(o) = owner.as_ref() {
            qb.push(" AND owner = ").push_bind(o);
        }
        if let Some(a) = audience.as_ref() {
            qb.push(" AND audience = ").push_bind(a);
        }
        if let Some(st) = status.as_ref() {
            qb.push(" AND status = ").push_bind(st);
        }
        qb.push(" ORDER BY owner, audience LIMIT ")
            .push_bind(size as i64);
        qb.push(" OFFSET ").push_bind(offset);
    })?;

    let total: (i64,) = db_qb_fetch_one!(pool, (i64,), |qb| {
        qb.push("SELECT COUNT(*) FROM token_service_jwt_grants WHERE 1=1");
        if let Some(o) = owner.as_ref() {
            qb.push(" AND owner = ").push_bind(o);
        }
        if let Some(a) = audience.as_ref() {
            qb.push(" AND audience = ").push_bind(a);
        }
        if let Some(st) = status.as_ref() {
            qb.push(" AND status = ").push_bind(st);
        }
    })?;

    Ok((rows, total.0))
}

/// create 的结果：`Inserted` 成功；`Duplicate` 已存在（含并发 PK 冲突）。
#[derive(Debug, PartialEq, Eq)]
pub enum CreateOutcome {
    Inserted,
    Duplicate,
}

/// 新建 grant（`active`, `system_managed=false`）。
///
/// 先查后插 + PK 唯一约束兜底：并发创建相同 `(owner, audience)` 时恰一次
/// 返回 `Inserted`，另一边 INSERT 报唯一键冲突映射为 `Duplicate`。
/// `owner`/`audience` 必须已通过 [`validate_service_identity`]（ops 层校验）。
pub async fn create_grant(
    pool: &DbPool,
    owner: &str,
    audience: &str,
) -> Result<CreateOutcome, sqlx::Error> {
    if find_by_owner_and_audience(pool, owner, audience)
        .await?
        .is_some()
    {
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
        Err(ref e) if is_duplicate_key(e) => Ok(CreateOutcome::Duplicate),
        Err(e) => Err(e),
    }
}

/// 启用 / 禁用。条件 UPDATE（`AND status <> ? AND system_managed = FALSE`）
/// 保证并发下后写者不会把已被他人改到目标状态的行"再次变更"，且
/// **system_managed 保护在写 SQL 内原子生效**（R1-02：先查后改的
/// TOCTOU 窗口内，即使行刚被并发置为 system_managed，UPDATE 也不会命中）
/// ——重复请求 / 系统保护行返回 `affected=0`，语义由调用方重查判定。
/// 返回受影响行数（0 = 行不存在、已处于目标状态，或受系统保护）。
pub async fn set_grant_status(
    pool: &DbPool,
    owner: &str,
    audience: &str,
    status: &str,
) -> Result<u64, sqlx::Error> {
    let affected = db_execute!(
        pool,
        "UPDATE token_service_jwt_grants SET status = ?, updated_at = ? \
         WHERE owner = ? AND audience = ? AND status <> ? AND system_managed = FALSE",
        status,
        chrono::Utc::now(),
        owner,
        audience,
        status
    )?;
    Ok(affected)
}

/// 删除 grant。条件 DELETE（`AND system_managed = FALSE`）把系统保护
/// 下沉到写 SQL 内原子生效（R1-02）：先查后删的 TOCTOU 窗口内被并发
/// 置为 system_managed 的行**不会**被删除。返回受影响行数（0 = 不存在、
/// 已删，或受系统保护），语义由调用方重查判定。
pub async fn delete_grant(pool: &DbPool, owner: &str, audience: &str) -> Result<u64, sqlx::Error> {
    let affected = db_execute!(
        pool,
        "DELETE FROM token_service_jwt_grants \
         WHERE owner = ? AND audience = ? AND system_managed = FALSE",
        owner,
        audience
    )?;
    Ok(affected)
}

// ── 事务内执行（WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-02 P1-02）────────────────
//
// 业务变更与成功审计必须同事务提交：任何一步失败整体回滚，不出现
// "业务已变更但审计缺失"。读走 `FOR UPDATE` 行锁（双方言语法一致），
// 把先查后改的 TOCTOU 窗口交给行锁闭合；system_managed 保护仍由
// 条件 UPDATE / DELETE 在 SQL 内原子兜底。

/// 事务内按主体+audience 取单行，`FOR UPDATE` 行锁至事务结束。
/// 供 set_status / delete 在变更前锁定目标行并读取 before 状态。
pub async fn find_by_owner_and_audience_for_update(
    tx: &mut Transaction<'_>,
    owner: &str,
    audience: &str,
) -> Result<Option<ServiceJwtGrantRow>, sqlx::Error> {
    let row: Option<ServiceJwtGrantRow> = db_tx_fetch_optional!(
        tx,
        ServiceJwtGrantRow,
        "SELECT owner, audience, status, system_managed, created_at, updated_at \
         FROM token_service_jwt_grants WHERE owner = ? AND audience = ? FOR UPDATE",
        owner,
        audience
    )?;
    Ok(row)
}

/// 事务内新建 grant（`active`, `system_managed=false`）。
///
/// 唯一主键兜底并发：两个事务并发 INSERT 相同 `(owner, audience)` 时，
/// 后者阻塞至前者提交 / 回滚——前者提交则后者报唯一键冲突映射
/// `Duplicate`（调用方回滚 → 409），前者回滚则后者正常插入。
/// 与池级 [`create_grant`] 的先查后插不同，事务内无需预查（冲突即真相，
/// 且预查在 READ COMMITTED / REPEATABLE READ 下都有竞态窗口）。
pub async fn create_grant_tx(
    tx: &mut Transaction<'_>,
    owner: &str,
    audience: &str,
) -> Result<CreateOutcome, sqlx::Error> {
    let res = db_tx_execute!(
        tx,
        "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
         VALUES (?, ?, 'active', FALSE)",
        owner,
        audience
    );
    match res {
        Ok(_) => Ok(CreateOutcome::Inserted),
        Err(ref e) if is_duplicate_key(e) => Ok(CreateOutcome::Duplicate),
        Err(e) => Err(e),
    }
}

/// 事务内启用 / 禁用。条件 UPDATE 与池级 [`set_grant_status`] 相同
///（`AND status <> ? AND system_managed = FALSE`）：调用方已在 FOR UPDATE
/// 锁下做过判定，此处条件仍原子兜底。返回受影响行数（0 = 未命中，
/// 调用方回滚并按异常处理）。
pub async fn set_grant_status_tx(
    tx: &mut Transaction<'_>,
    owner: &str,
    audience: &str,
    status: &str,
) -> Result<u64, sqlx::Error> {
    let affected = db_tx_execute!(
        tx,
        "UPDATE token_service_jwt_grants SET status = ?, updated_at = ? \
         WHERE owner = ? AND audience = ? AND status <> ? AND system_managed = FALSE",
        status,
        chrono::Utc::now(),
        owner,
        audience,
        status
    )?;
    Ok(affected)
}

/// 事务内删除 grant。条件 DELETE 与池级 [`delete_grant`] 相同
///（`AND system_managed = FALSE`）。返回受影响行数。
pub async fn delete_grant_tx(
    tx: &mut Transaction<'_>,
    owner: &str,
    audience: &str,
) -> Result<u64, sqlx::Error> {
    let affected = db_tx_execute!(
        tx,
        "DELETE FROM token_service_jwt_grants \
         WHERE owner = ? AND audience = ? AND system_managed = FALSE",
        owner,
        audience
    )?;
    Ok(affected)
}

/// 事务内写一行审计（append-only）。与业务变更同事务：审计失败 →
/// 调用方回滚业务变更（P1-02 原子性契约）。
#[allow(clippy::too_many_arguments)]
pub async fn record_audit_tx(
    tx: &mut Transaction<'_>,
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
    db_tx_execute!(
        tx,
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

// ── 审计（append-only 本地表，V006）─────────────────────────────────────────

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
/// `success` / `denied`；`detail` 为简短机器可读原因，**不得**包含
/// token / JWT / 凭据 / SQL。
#[allow(clippy::too_many_arguments)]
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
        qb.push(
            "SELECT id, actor_id, actor_type, owner, audience, action, before_status, \
                 after_status, request_id, result, detail, occurred_at \
                 FROM token_service_jwt_grant_audit WHERE 1=1",
        );
        if let Some(o) = owner.as_ref() {
            qb.push(" AND owner = ").push_bind(o);
        }
        if let Some(a) = audience.as_ref() {
            qb.push(" AND audience = ").push_bind(a);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_owner_strips_grace_suffix() {
        assert_eq!(canonical_owner("iam-service#old"), "iam-service");
        assert_eq!(canonical_owner("iam-service"), "iam-service");
        // 退化输入（裸 "#old"）剥成空串：空 owner 查不到任何 grant，
        // 走 fail-closed 拒绝——这正是期望行为。
        assert_eq!(canonical_owner("#old"), "");
    }

    #[test]
    fn service_identity_validation_matches_admin_owner_rules() {
        assert!(validate_service_identity("token-service").is_ok());
        assert!(validate_service_identity("prue_backend").is_ok());
        assert!(validate_service_identity("").is_err());
        assert!(validate_service_identity(&"a".repeat(65)).is_err());
        assert!(validate_service_identity("bad name").is_err());
        assert!(validate_service_identity("has#hash").is_err());
    }

    #[test]
    fn system_grants_have_no_wildcards_and_distinct_endpoints() {
        for (owner, audience) in SYSTEM_GRANTS {
            assert!(validate_service_identity(owner).is_ok());
            assert!(validate_service_identity(audience).is_ok());
            assert_ne!(owner, audience, "禁止隐式 owner==audience grant");
            assert!(
                !owner.contains('*') && !audience.contains('*'),
                "禁止通配符"
            );
        }
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

    #[test]
    fn audit_actions_are_stable_strings() {
        assert_eq!(AuditAction::Create.as_str(), "create");
        assert_eq!(AuditAction::Update.as_str(), "update");
        assert_eq!(AuditAction::Enable.as_str(), "enable");
        assert_eq!(AuditAction::Disable.as_str(), "disable");
        assert_eq!(AuditAction::Delete.as_str(), "delete");
    }
}
