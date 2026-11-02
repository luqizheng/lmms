use crate::db::DbPool;
use crate::models::oauth_client::{Client, ClientRow};
use crate::secret::{hash_client_secret, DEFAULT_DEV_CLIENT_SECRET};
use digua_common_db::{db_execute, db_fetch_optional, db_fetch_scalar, db_qb_fetch_all, db_qb_fetch_one};
use rand::distributions::Alphanumeric;
use rand::Rng;

/// `ClientRow` 的字段清单（唯一来源，避免多份 SELECT 漂移）。
///
/// 两侧方言都直接选原列：`permissions` / `grant_types` 由 sqlx 的
/// `sqlx::types::Json<T>` 解码（PG 侧列类型是 JSONB，与 `Json<T>` 的编码类型一致；
/// MySQL 侧是 JSON）。因此**不需要** `::text` / `CAST(... AS CHAR)` 这类转换。
const CLIENT_COLUMNS: &str = "client_id, client_secret_hash, name, permissions, grant_types, \
     access_token_ttl_sec, refresh_token_ttl_sec, status, user_id, expires_at, \
     created_at, updated_at";

#[derive(Debug, Clone)]
pub struct ClientPatch {
    pub name: Option<String>,
    pub permissions: Option<Vec<String>>,
    pub grant_types: Option<Vec<String>>,
    pub access_token_ttl_sec: Option<i64>,
    pub refresh_token_ttl_sec: Option<i64>,
    pub status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClientView {
    pub client_id: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub grant_types: Vec<String>,
    pub access_token_ttl_sec: i64,
    pub refresh_token_ttl_sec: i64,
    pub status: String,
    pub user_id: Option<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<ClientRow> for ClientView {
    fn from(r: ClientRow) -> Self {
        Self {
            client_id: r.client_id,
            name: r.name,
            permissions: r.permissions.0,
            grant_types: r.grant_types.0,
            access_token_ttl_sec: r.access_token_ttl_sec,
            refresh_token_ttl_sec: r.refresh_token_ttl_sec,
            status: r.status,
            user_id: r.user_id,
            expires_at: r.expires_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

pub fn generate_client_secret() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(40)
        .map(char::from)
        .collect()
}

pub fn generate_client_id(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let suffix: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .map(char::from)
        .collect::<String>()
        .to_lowercase();
    let trimmed = slug.trim_matches('-');
    let base = if trimmed.is_empty() {
        "client"
    } else {
        trimmed
    };
    format!("{}-{}", base, suffix)
}

pub async fn find_by_id(pool: &DbPool, client_id: &str) -> Result<Option<Client>, sqlx::Error> {
    let sql = format!("SELECT {CLIENT_COLUMNS} FROM token_oauth_clients WHERE client_id = ?");
    let row: Option<ClientRow> = db_fetch_optional!(pool, ClientRow, sql, client_id)?;
    Ok(row.map(|r| r.into_dto()))
}

pub async fn insert(pool: &DbPool, c: &Client) -> Result<(), sqlx::Error> {
    let perms = sqlx::types::Json(&c.permissions);
    let grants = sqlx::types::Json(&c.grant_types);
    db_execute!(
        pool,
        "INSERT INTO token_oauth_clients \
         (client_id, client_secret_hash, name, permissions, grant_types, \
          access_token_ttl_sec, refresh_token_ttl_sec, status, user_id, expires_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        &c.client_id,
        &c.client_secret_hash,
        &c.name,
        perms,
        grants,
        c.access_token_ttl_sec,
        c.refresh_token_ttl_sec,
        &c.status,
        &c.user_id,
        &c.expires_at
    )?;
    Ok(())
}

fn default_clients() -> Vec<Client> {
    let hash = hash_client_secret(DEFAULT_DEV_CLIENT_SECRET);
    vec![
        Client {
            client_id: "api-gateway".into(),
            client_secret_hash: hash.clone(),
            name: "API Gateway".into(),
            permissions: vec!["token:issue".into(), "token:revoke".into()],
            grant_types: vec!["authorization_code".into(), "refresh_token".into()],
            access_token_ttl_sec: 3600,
            refresh_token_ttl_sec: 2592000,
            status: "active".into(),
            user_id: None,
            expires_at: None,
        },
        Client {
            client_id: "token-service".into(),
            client_secret_hash: hash.clone(),
            name: "Token Service".into(),
            permissions: vec![
                "token:issue".into(),
                "token:revoke".into(),
                "token:refresh".into(),
                "user:read".into(),
            ],
            grant_types: vec!["authorization_code".into(), "refresh_token".into()],
            access_token_ttl_sec: 3600,
            refresh_token_ttl_sec: 2592000,
            status: "active".into(),
            user_id: None,
            expires_at: None,
        },
        Client {
            client_id: "user-service".into(),
            client_secret_hash: hash.clone(),
            name: "User Service".into(),
            permissions: vec![
                "user:read".into(),
                "user:write".into(),
                "role:read".into(),
                "permission:read".into(),
            ],
            grant_types: vec!["client_credentials".into()],
            access_token_ttl_sec: 3600,
            refresh_token_ttl_sec: 2592000,
            status: "active".into(),
            user_id: None,
            expires_at: None,
        },
        Client {
            client_id: "notification-service".into(),
            client_secret_hash: hash,
            name: "Notification Service".into(),
            permissions: vec!["notify:send".into(), "notify:read".into()],
            grant_types: vec!["client_credentials".into()],
            access_token_ttl_sec: 3600,
            refresh_token_ttl_sec: 2592000,
            status: "active".into(),
            user_id: None,
            expires_at: None,
        },
    ]
}

/// 空表时插入 4 条默认客户端；返回 affected rows。已存在则 0。
pub async fn seed_default_clients(pool: &DbPool) -> Result<u64, sqlx::Error> {
    let n: i64 = db_fetch_scalar!(pool, i64, "SELECT COUNT(*) FROM token_oauth_clients")?;
    if n > 0 {
        return Ok(0);
    }
    for c in default_clients() {
        insert(pool, &c).await?;
    }
    Ok(4)
}

pub async fn list(
    pool: &DbPool,
    keyword: Option<&str>,
    status: Option<&str>,
    page: u32,
    size: u32,
) -> Result<(Vec<ClientView>, i64), sqlx::Error> {
    let page = page.max(1);
    let size = size.clamp(1, 200);
    // `size` / `offset` 必须是 i64：sqlx-postgres 没有 u32 的 Encode 实现。
    let offset: i64 = ((page - 1) as i64) * (size as i64);
    let kw_pat = keyword
        .map(|k| format!("%{}%", k))
        .unwrap_or_else(|| "%".into());
    let st = status.map(|s| s.to_string());

    let rows: Vec<ClientRow> = db_qb_fetch_all!(pool, ClientRow, |qb| {
        qb.push("SELECT ")
            .push(CLIENT_COLUMNS)
            .push(" FROM token_oauth_clients WHERE 1=1");
        if st.is_some() {
            qb.push(" AND status = ").push_bind(st.as_ref());
        }
        qb.push(" AND (client_id LIKE ")
            .push_bind(&kw_pat)
            .push(" OR name LIKE ")
            .push_bind(&kw_pat)
            .push(")");
        qb.push(" ORDER BY created_at DESC LIMIT ")
            .push_bind(size as i64);
        qb.push(" OFFSET ").push_bind(offset);
    })?;

    let total: (i64,) = db_qb_fetch_one!(pool, (i64,), |qb| {
        qb.push("SELECT COUNT(*) FROM token_oauth_clients WHERE 1=1");
        if st.is_some() {
            qb.push(" AND status = ").push_bind(st.as_ref());
        }
        qb.push(" AND (client_id LIKE ")
            .push_bind(&kw_pat)
            .push(" OR name LIKE ")
            .push_bind(&kw_pat)
            .push(")");
    })?;

    Ok((rows.into_iter().map(ClientView::from).collect(), total.0))
}

pub async fn update(
    pool: &DbPool,
    client_id: &str,
    patch: &ClientPatch,
) -> Result<bool, sqlx::Error> {
    let current = find_by_id(pool, client_id).await?;
    let Some(mut c) = current else {
        return Ok(false);
    };
    if let Some(v) = &patch.name {
        c.name = v.clone();
    }
    if let Some(v) = &patch.permissions {
        c.permissions = v.clone();
    }
    if let Some(v) = &patch.grant_types {
        c.grant_types = v.clone();
    }
    if let Some(v) = patch.access_token_ttl_sec {
        c.access_token_ttl_sec = v;
    }
    if let Some(v) = patch.refresh_token_ttl_sec {
        c.refresh_token_ttl_sec = v;
    }
    if let Some(v) = &patch.status {
        c.status = v.clone();
    }
    let perms = sqlx::types::Json(&c.permissions);
    let grants = sqlx::types::Json(&c.grant_types);
    // ⚠️ `updated_at` 由应用层显式写：MySQL 侧过去靠列级
    // `ON UPDATE CURRENT_TIMESTAMP` 自动刷新，PG 无该语法，故两侧统一成显式赋值
    // （MySQL 也接受显式值并覆盖自动刷新，行为一致）。
    let affected = db_execute!(
        pool,
        "UPDATE token_oauth_clients SET name = ?, permissions = ?, grant_types = ?, \
         access_token_ttl_sec = ?, refresh_token_ttl_sec = ?, status = ?, updated_at = ? \
         WHERE client_id = ?",
        &c.name,
        perms,
        grants,
        c.access_token_ttl_sec,
        c.refresh_token_ttl_sec,
        &c.status,
        chrono::Utc::now(),
        client_id
    )?;
    Ok(affected > 0)
}

pub async fn delete(pool: &DbPool, client_id: &str) -> Result<bool, sqlx::Error> {
    let affected = db_execute!(
        pool,
        "DELETE FROM token_oauth_clients WHERE client_id = ?",
        client_id
    )?;
    Ok(affected > 0)
}

/// 重置 secret，返回明文 secret；未找到客户端时返回 None。
pub async fn reset_secret(pool: &DbPool, client_id: &str) -> Result<Option<String>, sqlx::Error> {
    let plain = generate_client_secret();
    let hash = hash_client_secret(&plain);
    // 同 `update`：`updated_at` 显式维护（PG 无列级 ON UPDATE）。
    let affected = db_execute!(
        pool,
        "UPDATE token_oauth_clients SET client_secret_hash = ?, updated_at = ? \
         WHERE client_id = ?",
        &hash,
        chrono::Utc::now(),
        client_id
    )?;
    if affected == 0 {
        Ok(None)
    } else {
        Ok(Some(plain))
    }
}
