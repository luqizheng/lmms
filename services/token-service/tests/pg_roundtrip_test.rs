//! token-service 在 **PostgreSQL** 上的读写往返验证。
//!
//! 本轮改造的实质目标：删掉本 crate 自建的只有 `Mysql` 变体的 `DbPool`，
//! 统一到基座层 `digua_common_db::DbPool`，并让**同一份 SQL 源码**在 PG 上跑通。
//!
//! 覆盖四类 PG 专属差异：
//! 1. `TIMESTAMPTZ` → `DateTime<Utc>` 解码（不是 TIMESTAMP）
//! 2. `JSONB` ↔ `sqlx::types::Json<T>`（写用 `Json<T>` 绑定、读用 `Json<T>` 解码）
//! 3. **`updated_at` 由应用层维护**——MySQL 侧原来是列级 `ON UPDATE CURRENT_TIMESTAMP`，
//!    PG 无此语法，必须靠 UPDATE 语句显式写 `updated_at = ?`（本文件断言它真的变了）
//! 4. 幂等插入：MySQL `INSERT IGNORE` → 裸 `INSERT` + 捕获重复键（SQLSTATE 23505）
//!
//! 走 **`TEST_PG_URL` 外部容器模式**（本环境 testcontainers 不可用）。
//! 无该变量时打印 `[skip]` 并直接返回，不失败。

#![cfg(feature = "postgres")]

use chrono::Utc;
use token_service::db::{self, DbPool};
use token_service::models::oauth_client::Client;
use token_service::models::service_jwt_grant as grant;
use token_service::service::{auth_code_svc, oauth_client_svc, refresh_svc, revoke_svc};

fn test_pg_url() -> Option<String> {
    match std::env::var("TEST_PG_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url),
        _ => {
            eprintln!("[token_pg_roundtrip] TEST_PG_URL not set — skipped");
            None
        }
    }
}

/// 从 `postgres://u:p@h:p/dbname` 里替换库名。
fn with_db(url: &str, db_name: &str) -> String {
    let (base, _old) = url
        .rsplit_once('/')
        .expect("TEST_PG_URL must contain a /db");
    format!("{base}/{db_name}")
}

/// 建一个唯一数据库并返回连到它的 `DbPool`（**未**跑迁移）。
async fn fresh_pg_pool() -> Option<DbPool> {
    let url = test_pg_url()?;
    let db_name = format!(
        "test_token_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );

    let admin = sqlx::PgPool::connect(&url).await.expect("admin connect pg");
    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin)
        .await
        .expect("create test database");
    admin.close().await;

    Some(
        digua_common_db::init_pool(&with_db(&url, &db_name))
            .await
            .expect("init_pool on fresh pg db"),
    )
}

/// 建库 + 跑完 migrations。
async fn fresh_pool() -> Option<DbPool> {
    let pool = fresh_pg_pool().await?;
    db::ensure_schema(&pool).await.expect("ensure_schema on pg");
    Some(pool)
}

fn sample_client(client_id: &str) -> Client {
    Client {
        client_id: client_id.to_string(),
        client_secret_hash: "hash-not-plaintext".to_string(),
        name: "网关客户端".to_string(),
        permissions: vec!["token:issue".to_string(), "token:revoke".to_string()],
        grant_types: vec!["client_credentials".to_string()],
        access_token_ttl_sec: 3600,
        refresh_token_ttl_sec: 2592000,
        status: "active".to_string(),
        user_id: Some("u-1".to_string()),
        expires_at: None,
    }
}

// ---------------------------------------------------------------------------
// 迁移层
// ---------------------------------------------------------------------------

/// `ensure_schema` 在 PG 上必须**幂等**：重复调用不报错、不重复建表。
#[tokio::test]
async fn pg_ensure_schema_is_idempotent() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };
    db::ensure_schema(&pool).await.expect("first apply");
    db::ensure_schema(&pool)
        .await
        .expect("second ensure_schema must be a no-op");
    db::ensure_schema(&pool)
        .await
        .expect("third ensure_schema must be a no-op");
}

/// V002 真的补跑过：`user_id` / `expires_at` 列与哨兵表都在，且类型是 PG 的对应类型。
#[tokio::test]
async fn pg_v002_columns_and_sentinel_exist() {
    let Some(pool) = fresh_pool().await else {
        return;
    };
    let pg = pool.as_postgres().expect("pg pool");

    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables \
         WHERE table_schema = current_schema() \
           AND table_name IN ('token_migration_v002_applied')",
    )
    .fetch_one(pg)
    .await
    .expect("probe sentinel");
    assert_eq!(row.0, 1, "V002 哨兵表必须存在");

    let cols: Vec<(String, String)> = sqlx::query_as(
        "SELECT column_name, data_type::text FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = 'token_oauth_clients' \
           AND column_name IN ('user_id', 'expires_at', 'created_at', 'updated_at', \
                               'permissions', 'grant_types') \
         ORDER BY column_name",
    )
    .fetch_all(pg)
    .await
    .expect("describe columns");

    let map: std::collections::HashMap<String, String> = cols.into_iter().collect();
    assert_eq!(
        map.get("user_id").map(String::as_str),
        Some("character varying")
    );
    // 时间列必须是 TIMESTAMPTZ（写错成 TIMESTAMP 会让 DateTime<Utc> 解码失败）
    for col in ["expires_at", "created_at", "updated_at"] {
        assert_eq!(
            map.get(col).map(String::as_str),
            Some("timestamp with time zone"),
            "{col} 必须是 TIMESTAMPTZ"
        );
    }
    // json 列是 JSONB（sqlx 的 Json<T> 以 JSONB 绑定参数）
    for col in ["permissions", "grant_types"] {
        assert_eq!(
            map.get(col).map(String::as_str),
            Some("jsonb"),
            "{col} 必须是 JSONB"
        );
    }
}

// ---------------------------------------------------------------------------
// 生产代码路径（不是手抄 SQL）
// ---------------------------------------------------------------------------

/// oauth client 的写入 / 读取 / JSON 往返 / 时间解码。
#[tokio::test]
async fn pg_oauth_client_roundtrip() {
    let Some(pool) = fresh_pool().await else {
        return;
    };

    let client = sample_client("网关-client-1");
    oauth_client_svc::insert(&pool, &client)
        .await
        .expect("insert client on PG");

    let found = oauth_client_svc::find_by_id(&pool, &client.client_id)
        .await
        .expect("find_by_id on PG")
        .expect("client must exist");

    assert_eq!(found.client_id, client.client_id);
    assert_eq!(found.name, "网关客户端", "中文必须原样往返");
    assert_eq!(
        found.permissions, client.permissions,
        "JSONB 列必须完整往返（Json<Vec<String>> 解码）"
    );
    assert_eq!(found.grant_types, client.grant_types);
    assert_eq!(found.access_token_ttl_sec, 3600);
    assert_eq!(found.user_id.as_deref(), Some("u-1"));
    assert_eq!(found.expires_at, None);

    // 字面量类型检查：走 ClientRow 解码，说明 created_at/updated_at 是真 DateTime<Utc>。
    // 用 SQL 再取一次，确认 TIMESTAMPTZ 的值接近当前 UTC 时间（会话时区已锁 UTC）。
    let pg = pool.as_postgres().expect("pg pool");
    let (row_ts,): (chrono::DateTime<Utc>,) =
        sqlx::query_as("SELECT updated_at FROM token_oauth_clients WHERE client_id = $1")
            .bind(&client.client_id)
            .fetch_one(pg)
            .await
            .expect("select updated_at");
    let skew = (Utc::now() - row_ts).num_seconds().abs();
    assert!(
        skew < 600,
        "updated_at 必须接近当前 UTC 时间（差 {skew}s），说明 TIMESTAMPTZ 解码正确"
    );
}

/// **核心用例**：`update` 必须让 `updated_at` 真的前进。
///
/// 这正是 MySQL `ON UPDATE CURRENT_TIMESTAMP` 的替代品：PG 没有列级自动更新，
/// 全靠 UPDATE 语句里显式写 `updated_at = ?`。若哪天有人把那个赋值删掉，
/// 这个用例立刻红。
#[tokio::test]
async fn pg_update_advances_updated_at() {
    let Some(pool) = fresh_pool().await else {
        return;
    };

    let client = sample_client("bump-2");
    oauth_client_svc::insert(&pool, &client)
        .await
        .expect("insert client");

    let (views, total) = oauth_client_svc::list(&pool, None, None, 1, 10)
        .await
        .expect("list on PG");
    assert_eq!(total, 1);
    let before = views[0].updated_at;

    // 让应用侧 Utc::now() 明显前进（避免同毫秒导致断言不稳）。
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    let updated = oauth_client_svc::update(
        &pool,
        &client.client_id,
        &oauth_client_svc::ClientPatch {
            name: Some("第二次改名".to_string()),
            permissions: Some(vec!["token:issue".to_string()]),
            grant_types: None,
            access_token_ttl_sec: None,
            refresh_token_ttl_sec: None,
            status: None,
        },
    )
    .await
    .expect("update on PG");
    assert!(updated, "update 必须命中 1 行");

    let (views, _) = oauth_client_svc::list(&pool, None, None, 1, 10)
        .await
        .expect("list after update");
    assert_eq!(views[0].name, "第二次改名", "patch 必须生效");
    assert_eq!(
        views[0].permissions,
        vec!["token:issue".to_string()],
        "permissions 必须被覆盖"
    );
    assert_eq!(
        views[0].grant_types,
        vec!["client_credentials".to_string()],
        "未 patch 的字段必须保留"
    );
    assert!(
        views[0].updated_at > before,
        "updated_at 必须前进（应用层显式维护，替代 MySQL 的 ON UPDATE）：{} -> {}",
        before,
        views[0].updated_at
    );
}

/// `list` 的关键字/状态过滤与分页（`size` / `offset` 必须 as i64）。
#[tokio::test]
async fn pg_list_filters_and_paginates() {
    let Some(pool) = fresh_pool().await else {
        return;
    };

    for (id, status) in [
        ("svc-a", "active"),
        ("svc-b", "disabled"),
        ("其它-c", "active"),
    ] {
        let mut c = sample_client(id);
        c.status = status.to_string();
        oauth_client_svc::insert(&pool, &c).await.expect("insert");
    }

    let (all, total) = oauth_client_svc::list(&pool, None, None, 1, 10)
        .await
        .expect("list all");
    assert_eq!(total, 3);
    assert_eq!(all.len(), 3);

    let (page2, total) = oauth_client_svc::list(&pool, None, None, 2, 2)
        .await
        .expect("list page 2");
    assert_eq!(total, 3);
    assert_eq!(page2.len(), 1, "LIMIT/OFFSET 必须在 PG 上生效");

    let (only_active, total) = oauth_client_svc::list(&pool, None, Some("active"), 1, 10)
        .await
        .expect("list by status");
    assert_eq!(total, 2);
    assert!(only_active.iter().all(|c| c.status == "active"));

    let (by_kw, total) = oauth_client_svc::list(&pool, Some("svc"), None, 1, 10)
        .await
        .expect("list by keyword");
    assert_eq!(total, 2, "LIKE 过滤必须生效");
    assert!(by_kw.iter().all(|c| c.client_id.starts_with("svc")));
}

/// seed / delete / reset_secret 在 PG 上同样成立。
#[tokio::test]
async fn pg_seed_delete_reset_secret() {
    let Some(pool) = fresh_pool().await else {
        return;
    };

    let seeded = oauth_client_svc::seed_default_clients(&pool)
        .await
        .expect("seed on PG");
    assert_eq!(seeded, 4, "空表应插入 4 条默认客户端");

    // 幂等：再跑一次不插
    let again = oauth_client_svc::seed_default_clients(&pool)
        .await
        .expect("seed again");
    assert_eq!(again, 0);

    let found = oauth_client_svc::find_by_id(&pool, "api-gateway")
        .await
        .expect("find seeded")
        .expect("api-gateway 必须存在");
    assert_eq!(found.name, "API Gateway");
    assert_eq!(
        found.permissions,
        vec!["token:issue".to_string(), "token:revoke".to_string()]
    );

    // reset_secret：命中 → 返回明文；再查一次 hash 必须变
    let plain = oauth_client_svc::reset_secret(&pool, "api-gateway")
        .await
        .expect("reset_secret on PG")
        .expect("命中");
    assert_eq!(plain.len(), 40);
    let after = oauth_client_svc::find_by_id(&pool, "api-gateway")
        .await
        .expect("find after reset")
        .expect("exists");
    assert_ne!(after.client_secret_hash, found.client_secret_hash);

    assert!(oauth_client_svc::reset_secret(&pool, "no-such-client")
        .await
        .expect("reset_secret miss")
        .is_none());

    assert!(oauth_client_svc::delete(&pool, "api-gateway")
        .await
        .expect("delete on PG"));
    assert!(!oauth_client_svc::delete(&pool, "api-gateway")
        .await
        .expect("delete again"));
}

/// 授权码：写入 → 消费一次成功 → 二次消费失败；重复 code 写入幂等。
#[tokio::test]
async fn pg_auth_code_lifecycle() {
    let Some(pool) = fresh_pool().await else {
        return;
    };

    auth_code_svc::save(&pool, "code-1", "user-1", 300)
        .await
        .expect("save auth code");
    // 重复 code 必须是 no-op（MySQL 23000 / PG 23505 都被吞掉）
    auth_code_svc::save(&pool, "code-1", "user-1", 300)
        .await
        .expect("duplicate save must be a no-op");

    let first = auth_code_svc::consume(&pool, "code-1")
        .await
        .expect("consume on PG");
    assert_eq!(first.as_deref(), Some("user-1"));

    let second = auth_code_svc::consume(&pool, "code-1")
        .await
        .expect("re-consume");
    assert!(second.is_none(), "已消费的 code 不能再次消费");

    // 已过期的 code 不可消费
    auth_code_svc::save(&pool, "code-expired", "user-2", -1)
        .await
        .expect("save expired code");
    assert!(
        auth_code_svc::consume(&pool, "code-expired")
            .await
            .expect("consume expired")
            .is_none(),
        "过期 code 不能消费"
    );
}

/// refresh 标记 / 吊销名单：写入幂等 + 命中判断。
#[tokio::test]
async fn pg_refresh_and_revoke_markers() {
    let Some(pool) = fresh_pool().await else {
        return;
    };

    // refresh
    assert!(!refresh_svc::is_marked(&pool, "jti-1").await.expect("probe"));
    refresh_svc::mark(&pool, "jti-1", 300).await.expect("mark");
    assert!(refresh_svc::is_marked(&pool, "jti-1").await.expect("probe"));
    // 重复 mark 必须幂等（替代 MySQL 的 INSERT IGNORE）
    refresh_svc::mark(&pool, "jti-1", 300)
        .await
        .expect("duplicate mark must be a no-op");
    // 已过期的不算
    refresh_svc::mark(&pool, "jti-expired", -1)
        .await
        .expect("mark expired");
    assert!(
        !refresh_svc::is_marked(&pool, "jti-expired")
            .await
            .expect("probe expired"),
        "过期标记不应命中"
    );

    // revoke
    assert!(!revoke_svc::is_revoked(&pool, "jti-2").await.expect("probe"));
    revoke_svc::revoke(&pool, "jti-2", 300)
        .await
        .expect("revoke");
    assert!(revoke_svc::is_revoked(&pool, "jti-2").await.expect("probe"));
    revoke_svc::revoke(&pool, "jti-2", 300)
        .await
        .expect("duplicate revoke must be a no-op");
}

// ---------------------------------------------------------------------------
// Service JWT audience grants（WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01）
// ---------------------------------------------------------------------------

/// grants 全生命周期在 PG 上的往返：上游 code-owned seed → 管理面 create
/// （重复冲突 23505 映射 Duplicate）→ set_grant_status（updated_at 应用层
/// 维护）→ is_allowed → delete → 审计写入 / 读取。
#[tokio::test]
async fn pg_service_jwt_grants_roundtrip() {
    let Some(pool) = fresh_pool().await else {
        return;
    };

    // 上游 WP-02 code-owned seed：精准 SYSTEM_GRANTS（2 条），幂等
    let seeded = grant::ensure_system_grants(&pool)
        .await
        .expect("seed system grants on pg");
    assert_eq!(seeded, 2);
    let again = grant::ensure_system_grants(&pool)
        .await
        .expect("re-seed");
    assert_eq!(again, 0, "seed must be idempotent");

    // 系统行 active 且受保护
    assert!(grant::is_allowed(&pool, "iam-service", "token-service")
        .await
        .expect("probe system grant"));
    let sys_row = grant::find_by_owner_and_audience(&pool, "iam-service", "token-service")
        .await
        .expect("find system row")
        .expect("system row exists");
    assert!(sys_row.system_managed);

    // create：成功 + 重复映射 Duplicate
    assert_eq!(
        grant::create_grant(&pool, "svc-x", "audit-service")
            .await
            .expect("create"),
        grant::CreateOutcome::Inserted
    );
    assert_eq!(
        grant::create_grant(&pool, "svc-x", "audit-service")
            .await
            .expect("duplicate create"),
        grant::CreateOutcome::Duplicate,
        "PG 23505 必须映射为 Duplicate"
    );

    // set_status：disabled → 查询失活；updated_at 前进（应用层维护）
    let before = grant::find_by_owner_and_audience(&pool, "svc-x", "audit-service")
        .await
        .expect("find")
        .expect("row");
    assert_eq!(before.status, grant::STATUS_ACTIVE);
    assert!(!before.system_managed);
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let affected = grant::set_grant_status(&pool, "svc-x", "audit-service", grant::STATUS_DISABLED)
        .await
        .expect("set_status");
    assert_eq!(affected, 1);
    assert!(
        !grant::is_allowed(&pool, "svc-x", "audit-service")
            .await
            .expect("probe disabled"),
        "disabled 后签发判定必须失活"
    );
    let after = grant::find_by_owner_and_audience(&pool, "svc-x", "audit-service")
        .await
        .expect("find after")
        .expect("row");
    assert!(
        after.updated_at > before.updated_at,
        "PG 无 ON UPDATE CURRENT_TIMESTAMP，updated_at 必须由应用层推进"
    );

    // list_filtered：过滤 + 分页 + 稳定排序（svc-x 只有本测试 create 的一行；
    // 上游 WP-02 语义下没有 env seed，系统 seed 不含 svc-x）
    let (rows, total) = grant::list_filtered(&pool, Some("svc-x"), None, None, 1, 10)
        .await
        .expect("list");
    assert_eq!(total, 1);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].audience, "audit-service");
    // 系统行可见 + 全表分页生效
    let (_, sys_total) = grant::list_filtered(&pool, None, Some("token-service"), None, 1, 10)
        .await
        .expect("list by audience");
    assert_eq!(sys_total, 1, "系统 grant（iam→token）可按 audience 过滤");
    let (page1, total_all) = grant::list_filtered(&pool, None, None, None, 1, 2)
        .await
        .expect("page 1 of all");
    assert_eq!(total_all, 3, "2 system + 1 created");
    assert_eq!(page1.len(), 2, "LIMIT/OFFSET 在 PG 上生效");
    let (page2, _) = grant::list_filtered(&pool, None, None, None, 2, 2)
        .await
        .expect("page 2");
    assert_eq!(page2.len(), 1, "LIMIT/OFFSET 在 PG 上生效");

    // 审计写入 / 读取（append-only）
    grant::record_audit(
        &pool,
        &grant::Actor::User {
            user_id: "usr-pg".to_string(),
        },
        "svc-x",
        "audit-service",
        grant::AuditAction::Disable,
        Some(grant::STATUS_ACTIVE),
        Some(grant::STATUS_DISABLED),
        "req-pg-1",
        "success",
        "status updated",
    )
    .await
    .expect("record audit on pg");
    let audits = grant::list_audit(&pool, Some("svc-x"), Some("audit-service"), 10)
        .await
        .expect("list audit");
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].action, "disable");
    assert_eq!(audits[0].actor_type, "user");
    assert_eq!(audits[0].request_id, "req-pg-1");
    assert_eq!(audits[0].after_status.as_deref(), Some("disabled"));

    // delete：再次删除 affected = 0
    let affected = grant::delete_grant(&pool, "svc-x", "audit-service")
        .await
        .expect("delete");
    assert_eq!(affected, 1);
    let affected = grant::delete_grant(&pool, "svc-x", "audit-service")
        .await
        .expect("delete again");
    assert_eq!(affected, 0, "已删行再次删除必须命中 0 行");
}
