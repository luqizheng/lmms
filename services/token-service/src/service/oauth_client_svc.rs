use crate::db::DbPool;
use crate::models::oauth_client::{Client, ClientRow};
use crate::secret::{hash_client_secret, DEFAULT_DEV_CLIENT_SECRET};
use rand::distributions::Alphanumeric;
use rand::Rng;

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
    let base = if trimmed.is_empty() { "client" } else { trimmed };
    format!("{}-{}", base, suffix)
}

pub async fn find_by_id(pool: &DbPool, client_id: &str) -> Result<Option<Client>, sqlx::Error> {
    match pool {
        DbPool::Mysql(p) => {
            let row: Option<ClientRow> = sqlx::query_as(
                "SELECT client_id, client_secret_hash, name, permissions, grant_types, \
                        access_token_ttl_sec, refresh_token_ttl_sec, status, created_at, updated_at \
                 FROM token_oauth_clients WHERE client_id = ?",
            )
            .bind(client_id)
            .fetch_optional(p)
            .await?;
            Ok(row.map(|r| r.into_dto()))
        }
    }
}

pub async fn insert(pool: &DbPool, c: &Client) -> Result<(), sqlx::Error> {
    match pool {
        DbPool::Mysql(p) => {
            let perms = sqlx::types::Json(&c.permissions);
            let grants = sqlx::types::Json(&c.grant_types);
            sqlx::query(
                "INSERT INTO token_oauth_clients \
                 (client_id, client_secret_hash, name, permissions, grant_types, \
                  access_token_ttl_sec, refresh_token_ttl_sec, status) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&c.client_id)
            .bind(&c.client_secret_hash)
            .bind(&c.name)
            .bind(perms)
            .bind(grants)
            .bind(c.access_token_ttl_sec)
            .bind(c.refresh_token_ttl_sec)
            .bind(&c.status)
            .execute(p)
            .await?;
            Ok(())
        }
    }
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
        },
    ]
}

/// 空表时插入 4 条默认客户端；返回 affected rows。已存在则 0。
pub async fn seed_default_clients(pool: &DbPool) -> Result<u64, sqlx::Error> {
    let n: (i64,) = match pool {
        DbPool::Mysql(p) => {
            sqlx::query_as("SELECT COUNT(*) FROM token_oauth_clients")
                .fetch_one(p)
                .await?
        }
    };
    if n.0 > 0 {
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
    let offset: i64 = ((page - 1) as i64) * (size as i64);
    let kw_pat = keyword
        .map(|k| format!("%{}%", k))
        .unwrap_or_else(|| "%".into());
    let st = status.map(|s| s.to_string());

    match pool {
        DbPool::Mysql(p) => {
            let (rows, total): (Vec<ClientRow>, i64) = if let Some(ref s) = st {
                let rows: Vec<ClientRow> = sqlx::query_as(
                    "SELECT client_id, client_secret_hash, name, permissions, grant_types, \
                            access_token_ttl_sec, refresh_token_ttl_sec, status, created_at, updated_at \
                     FROM token_oauth_clients \
                     WHERE (client_id LIKE ? OR name LIKE ?) AND status = ? \
                     ORDER BY created_at DESC LIMIT ? OFFSET ?",
                )
                .bind(&kw_pat)
                .bind(&kw_pat)
                .bind(s)
                .bind(size as i64)
                .bind(offset)
                .fetch_all(p)
                .await?;
                let total: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM token_oauth_clients \
                     WHERE (client_id LIKE ? OR name LIKE ?) AND status = ?",
                )
                .bind(&kw_pat)
                .bind(&kw_pat)
                .bind(s)
                .fetch_one(p)
                .await?;
                (rows, total.0)
            } else {
                let rows: Vec<ClientRow> = sqlx::query_as(
                    "SELECT client_id, client_secret_hash, name, permissions, grant_types, \
                            access_token_ttl_sec, refresh_token_ttl_sec, status, created_at, updated_at \
                     FROM token_oauth_clients \
                     WHERE client_id LIKE ? OR name LIKE ? \
                     ORDER BY created_at DESC LIMIT ? OFFSET ?",
                )
                .bind(&kw_pat)
                .bind(&kw_pat)
                .bind(size as i64)
                .bind(offset)
                .fetch_all(p)
                .await?;
                let total: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM token_oauth_clients \
                     WHERE client_id LIKE ? OR name LIKE ?",
                )
                .bind(&kw_pat)
                .bind(&kw_pat)
                .fetch_one(p)
                .await?;
                (rows, total.0)
            };
            Ok((rows.into_iter().map(ClientView::from).collect(), total))
        }
    }
}

pub async fn update(
    pool: &DbPool,
    client_id: &str,
    patch: &ClientPatch,
) -> Result<bool, sqlx::Error> {
    match pool {
        DbPool::Mysql(p) => {
            let current = find_by_id(&DbPool::Mysql(p.clone()), client_id).await?;
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
            let res = sqlx::query(
                "UPDATE token_oauth_clients SET name = ?, permissions = ?, grant_types = ?, \
                 access_token_ttl_sec = ?, refresh_token_ttl_sec = ?, status = ? \
                 WHERE client_id = ?",
            )
            .bind(&c.name)
            .bind(perms)
            .bind(grants)
            .bind(c.access_token_ttl_sec)
            .bind(c.refresh_token_ttl_sec)
            .bind(&c.status)
            .bind(client_id)
            .execute(p)
            .await?;
            Ok(res.rows_affected() > 0)
        }
    }
}

pub async fn delete(pool: &DbPool, client_id: &str) -> Result<bool, sqlx::Error> {
    match pool {
        DbPool::Mysql(p) => {
            let res = sqlx::query("DELETE FROM token_oauth_clients WHERE client_id = ?")
                .bind(client_id)
                .execute(p)
                .await?;
            Ok(res.rows_affected() > 0)
        }
    }
}

/// 重置 secret，返回明文 secret；未找到客户端时返回 None。
pub async fn reset_secret(
    pool: &DbPool,
    client_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    let plain = generate_client_secret();
    let hash = hash_client_secret(&plain);
    match pool {
        DbPool::Mysql(p) => {
            let res =
                sqlx::query("UPDATE token_oauth_clients SET client_secret_hash = ? WHERE client_id = ?")
                    .bind(&hash)
                    .bind(client_id)
                    .execute(p)
                    .await?;
            if res.rows_affected() == 0 {
                Ok(None)
            } else {
                Ok(Some(plain))
            }
        }
    }
}
