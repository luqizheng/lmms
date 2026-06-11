use crate::db::DbPool;
use crate::models::oauth_client::{Client, ClientRow};
use crate::secret::{hash_client_secret, DEFAULT_DEV_CLIENT_SECRET};

pub async fn find_by_id(
    pool: &DbPool,
    client_id: &str,
) -> Result<Option<Client>, sqlx::Error> {
    match pool {
        DbPool::Mysql(p) => {
            let row: Option<ClientRow> = sqlx::query_as(
                "SELECT client_id, client_secret_hash, name, permissions, grant_types, \
                        access_token_ttl_sec, refresh_token_ttl_sec, status, created_at, updated_at \
                 FROM oauth_clients WHERE client_id = ?",
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
                "INSERT INTO oauth_clients \
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
            sqlx::query_as("SELECT COUNT(*) FROM oauth_clients")
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
