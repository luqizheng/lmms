use crate::db::DbPool;
use digua_common_db::{db_execute, db_fetch_optional, db_fetch_scalar};
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::borrow::Cow;

/// seed 用的一对 `(owner, 明文 token)`。
type SeedPair = (Cow<'static, str>, Cow<'static, str>);

/// 内置 dev 默认 bootstrap token 对（owner → 明文 token）。
///
/// 用途：**env 未配置时也能开箱跑通 dev 栈**——否则 `token_bootstrap_tokens`
/// 表为空，所有业务服务调 `POST /tokens/service-jwt` 都会拿到 401。
/// 值与 `infrastructure/lmms-dev/.env` 的 dev 固定值保持一致。
///
/// ⚠️ 仅限 dev。生产环境应通过 env 注入真 token 并覆盖本表。
pub const DEFAULT_DEV_BOOTSTRAP: &[(&str, &str)] = &[
    ("iam-service", "dev-bt-iam-service"),
    ("token-service", "dev-bt-token-service"),
    ("notification-service", "dev-bt-notification-service"),
    ("audit-service", "dev-bt-audit-service"),
    ("prue_backend", "dev-bt-prue-backend"),
    ("api-gateway", "dev-bt-api-gateway"),
    ("file-service", "dev-bt-file-service"),
];

/// bootstrap token 表的行结构（Phase 5-B 演进：从 env CSV 迁移到 DB）。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BootstrapTokenRow {
    pub owner: String,
    /// 当前阶段存**明文**；列名 token_hash 预留以便 Phase 3 rotate 上线时
    /// 无痛切 SHA-256 —— 只改写入路径，列名 / 索引 / 查询 SQL 全部不变。
    pub token_hash: String,
    pub status: String,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// 根据明文 token 查出 owner（用于 service_jwt handler 校验）。
///
/// 查询条件：
///   1. token_hash 精确匹配（当前阶段 token_hash = 明文，未来切 sha256 后仍兼容）；
///   2. status = 'active'；
///   3. expires_at IS NULL 或 expires_at > NOW()。
pub async fn find_active_owner_by_token(
    pool: &DbPool,
    plain: &str,
) -> Result<Option<String>, sqlx::Error> {
    // 当前阶段 token_hash 列直接存明文；Phase 3 rotate 上线时改传 sha256
    let token_value = plain;

    let row: Option<(String,)> = db_fetch_optional!(
        pool,
        (String,),
        "SELECT owner FROM token_bootstrap_tokens \
         WHERE token_hash = ? \
           AND status = 'active' \
           AND (expires_at IS NULL OR expires_at > NOW())",
        token_value
    )?;
    Ok(row.map(|(owner,)| owner))
}

/// 解析 env 的两个 CSV → `(owner, token)` 列表。
///
/// 策略（**fail-closed**）：
///   - env 两个 CSV 都为空 → 用内置 dev 默认值（dev 开箱即用）；
///   - env 提供了 tokens 但 owners 缺失 / 长度不匹配 → **返回 Err，拒绝启动**。
///     绝不能回退到内置默认值 —— 那会让生产环境静默用上硬编码 dev token。
fn resolve_seed_pairs(tokens_env: &str, owners_env: &str) -> Result<Vec<SeedPair>, String> {
    let tokens: Vec<&str> = tokens_env
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let owners: Vec<&str> = owners_env
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    if tokens.is_empty() && owners.is_empty() {
        tracing::warn!(
            "SERVICE_BOOTSTRAP_TOKENS / SERVICE_BOOTSTRAP_OWNERS not set; \
             falling back to built-in dev defaults ({} owners)",
            DEFAULT_DEV_BOOTSTRAP.len()
        );
        return Ok(DEFAULT_DEV_BOOTSTRAP
            .iter()
            .map(|(o, t)| (Cow::Borrowed(*o), Cow::Borrowed(*t)))
            .collect());
    }
    if tokens.len() != owners.len() {
        return Err(format!(
            "SERVICE_BOOTSTRAP_TOKENS has {} entries but SERVICE_BOOTSTRAP_OWNERS has {} \
             — they must be equal length and aligned one-to-one (order matters). \
             Refusing to fall back to built-in dev defaults.",
            tokens.len(),
            owners.len()
        ));
    }

    Ok(owners
        .into_iter()
        .zip(tokens)
        .map(|(owner, token)| (Cow::Owned(owner.to_string()), Cow::Owned(token.to_string())))
        .collect())
}

/// 启动时 seed：把 `(owner, token)` 对写入表。
///
/// 语义：**按 owner 补插**——表里没有该 owner 才 INSERT，已有的行不动。
/// 这样既能首次灌入默认数据，又不会覆盖运维已 rotate 过的 token。
///
/// 幂等：重复启动不会重复插入（owner 是主键 + 先查后插）。
/// 返回实际插入行数。
pub async fn seed_defaults(
    pool: &DbPool,
    tokens_env: &str,
    owners_env: &str,
) -> Result<u64, sqlx::Error> {
    let pairs = resolve_seed_pairs(tokens_env, owners_env).map_err(|e| {
        // 用一个可还原的 sqlx 错误承载配置错误，调用方 expect 时能拿到 msg
        sqlx::Error::Protocol(format!("bootstrap token config invalid: {e}"))
    })?;

    let mut inserted = 0u64;
    for (owner, token) in pairs {
        let existing: i64 = db_fetch_scalar!(
            pool,
            i64,
            "SELECT COUNT(*) FROM token_bootstrap_tokens WHERE owner = ?",
            owner.as_ref()
        )?;
        if existing > 0 {
            continue;
        }
        db_execute!(
            pool,
            "INSERT INTO token_bootstrap_tokens (owner, token_hash, status) \
             VALUES (?, ?, 'active')",
            owner.as_ref(),
            token.as_ref()
        )?;
        inserted += 1;
    }
    Ok(inserted)
}

// ─────────────────────────────────────────────────────────────────────────────
// 管理操作（Phase 3：rotate / 禁用 / 查询）
//
// ⚠️ 全部用「先查后写」而非 `INSERT ... ON DUPLICATE KEY UPDATE`——
// 那是 MySQL 专有语法，PostgreSQL 侧必须写 `ON CONFLICT`，而 common-db 的
// `db_execute!` 宏不做方言转换。查一次 + 写一次在管理面（低频）完全够用。
// ─────────────────────────────────────────────────────────────────────────────

/// 列出全部 bootstrap token（**不返回明文 token**）。
pub async fn list_all(pool: &DbPool) -> Result<Vec<BootstrapTokenRow>, sqlx::Error> {
    let rows: Vec<BootstrapTokenRow> = digua_common_db::db_fetch_all!(
        pool,
        BootstrapTokenRow,
        "SELECT owner, token_hash, status, expires_at, last_used_at, created_at, updated_at \
         FROM token_bootstrap_tokens ORDER BY owner"
    )?;
    Ok(rows)
}

/// 按 owner 取单行。
pub async fn find_by_owner(
    pool: &DbPool,
    owner: &str,
) -> Result<Option<BootstrapTokenRow>, sqlx::Error> {
    let row: Option<BootstrapTokenRow> = db_fetch_optional!(
        pool,
        BootstrapTokenRow,
        "SELECT owner, token_hash, status, expires_at, last_used_at, created_at, updated_at \
         FROM token_bootstrap_tokens WHERE owner = ?",
        owner
    )?;
    Ok(row)
}

/// 生成 40 位随机 token（与 oauth_client_svc::generate_client_secret 同规格）。
pub fn generate_token() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(40)
        .map(char::from)
        .collect()
}

/// 新建某个 owner 的 bootstrap token，返回明文。
///
/// `owner` 已存在时返回 `Ok(None)`（调用方应改用 rotate）。
pub async fn create(
    pool: &DbPool,
    owner: &str,
    plain: &str,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<bool, sqlx::Error> {
    if find_by_owner(pool, owner).await?.is_some() {
        return Ok(false);
    }
    db_execute!(
        pool,
        "INSERT INTO token_bootstrap_tokens (owner, token_hash, status, expires_at) \
         VALUES (?, ?, 'active', ?)",
        owner,
        plain,
        &expires_at
    )?;
    Ok(true)
}

/// rotate：给 owner 换一个新 token，返回 `(新明文 token, 旧 token 失效时间)`。
///
/// `grace_period_sec > 0` → 旧 token 保留到 `now + grace`（滚动发布用：
/// 新旧 token 并存，服务可逐个重启切换）；`= 0` → 旧 token **立即失效**。
///
/// 实现说明：宽限期需要旧 token 与新 token **同时**能被校验，因此旧 token
/// 另起一行（owner 加 `#old` 后缀）存放，`status='active'` + `expires_at`
/// 到期后自然失效。owner 是主键，故用后缀避免冲突。
pub async fn rotate(
    pool: &DbPool,
    owner: &str,
    grace_period_sec: i64,
) -> Result<Option<(String, Option<chrono::DateTime<chrono::Utc>>)>, sqlx::Error> {
    let Some(existing) = find_by_owner(pool, owner).await? else {
        return Ok(None);
    };

    let new_plain = generate_token();
    let now = chrono::Utc::now();
    let old_expiry = if grace_period_sec > 0 {
        Some(now + chrono::Duration::seconds(grace_period_sec))
    } else {
        None
    };

    if grace_period_sec > 0 {
        // 旧 token 移到 `#old` 行，带 expires_at（到期后校验 SQL 自动拒绝）。
        let grace_owner = format!("{owner}#old");
        // 若上一轮 rotate 已留下 `#old` 行，先清掉（避免主键冲突）。
        db_execute!(
            pool,
            "DELETE FROM token_bootstrap_tokens WHERE owner = ?",
            &grace_owner
        )?;
        db_execute!(
            pool,
            "INSERT INTO token_bootstrap_tokens (owner, token_hash, status, expires_at) \
             VALUES (?, ?, 'active', ?)",
            &grace_owner,
            &existing.token_hash,
            &old_expiry
        )?;
    }

    // 正式行换成新 token，清掉 expires_at（永久有效）。
    db_execute!(
        pool,
        "UPDATE token_bootstrap_tokens \
         SET token_hash = ?, expires_at = NULL, status = 'active', updated_at = ? \
         WHERE owner = ?",
        &new_plain,
        now,
        owner
    )?;

    Ok(Some((new_plain, old_expiry)))
}

/// 禁用 / 启用某个 owner 的 token。
pub async fn set_status(pool: &DbPool, owner: &str, status: &str) -> Result<bool, sqlx::Error> {
    let affected = db_execute!(
        pool,
        "UPDATE token_bootstrap_tokens SET status = ?, updated_at = ? WHERE owner = ?",
        status,
        chrono::Utc::now(),
        owner
    )?;
    Ok(affected > 0)
}

/// 删除某个 owner 的行（含宽限期遗留的 `#old` 行）。
pub async fn delete(pool: &DbPool, owner: &str) -> Result<bool, sqlx::Error> {
    let affected = db_execute!(
        pool,
        "DELETE FROM token_bootstrap_tokens WHERE owner = ? OR owner = ?",
        owner,
        format!("{owner}#old")
    )?;
    Ok(affected > 0)
}

/// 清理已过期的宽限期遗留行（`#old` 后缀且 expires_at 已到）。
pub async fn purge_expired_grace_rows(pool: &DbPool) -> Result<u64, sqlx::Error> {
    let affected = db_execute!(
        pool,
        "DELETE FROM token_bootstrap_tokens \
         WHERE owner LIKE '%#old' AND expires_at IS NOT NULL AND expires_at <= NOW()"
    )?;
    Ok(affected as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_empty_falls_back_to_built_in_defaults() {
        let pairs = resolve_seed_pairs("", "").expect("fallback");
        assert_eq!(pairs.len(), DEFAULT_DEV_BOOTSTRAP.len());
        assert!(pairs
            .iter()
            .any(|(o, t)| { o.as_ref() == "iam-service" && t.as_ref() == "dev-bt-iam-service" }));
    }

    #[test]
    fn tokens_without_owners_is_rejected() {
        // fail-closed：生产只配了 tokens 没配 owners 时必须拒绝启动，
        // 绝不能静默回退到硬编码 dev token。
        let err = resolve_seed_pairs("a,b,c", "").expect_err("should reject");
        assert!(err.contains("must be equal length"));
    }

    #[test]
    fn mismatched_length_is_rejected() {
        let err = resolve_seed_pairs("a,b,c", "only-one").expect_err("should reject");
        assert!(err.contains("must be equal length"));
    }

    #[test]
    fn env_pairs_win_over_built_in_defaults() {
        let pairs = resolve_seed_pairs("tok-a,tok-b", "svc-a,svc-b").expect("env pairs");
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].0.as_ref(), "svc-a");
        assert_eq!(pairs[0].1.as_ref(), "tok-a");
        assert_eq!(pairs[1].0.as_ref(), "svc-b");
        assert_eq!(pairs[1].1.as_ref(), "tok-b");
    }

    #[test]
    fn env_pairs_ignore_blank_entries() {
        let pairs = resolve_seed_pairs("tok-a,,tok-b", "svc-a,svc-b").expect("env pairs");
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].0.as_ref(), "svc-a");
    }
}
