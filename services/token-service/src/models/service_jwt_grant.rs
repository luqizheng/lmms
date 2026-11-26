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
use digua_common_db::{db_execute, db_fetch_all, db_fetch_optional, db_fetch_scalar};

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
}
