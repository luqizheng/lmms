//! iam-service 在 **PostgreSQL** 上的数据层往返验证。
//!
//! 与 audit/token/notification 的 `pg_roundtrip_test.rs` 同一模式：
//! `TEST_PG_URL` 外部容器模式，未设时打印 `[skip]` 直接通过。
//! 每个用例建唯一数据库（`CREATE DATABASE`），避免并行测试互相污染。
//!
//! 服务层逻辑（同一份源码）已由 MySQL fixture 的 12 个集成套件覆盖，
//! 这里只验证 **PG 侧数据层**：
//! 1. `ensure_schema` 在 PG 上应用全部 V001~V008（marker 探测走 current_schema()）且幂等
//! 2. `db_*!` 宏对 iam 表的中文/bool/TIMESTAMPTZ 读写往返
//! 3. internal_membership 的 `STRING_AGG` 输出格式与 MySQL `GROUP_CONCAT` 一致
//!    （逗号分隔、无空格——网关/前端按此格式解析）
//! 4. tenant 自增 id 在 PG 走 `RETURNING id`（有符号 BIGINT ↔ i64）
//!
//! 断言用的裸 SQL 是 PG 方言（`$N` / `::text` / `->>`），经 `as_postgres()` 直连。

#![cfg(feature = "postgres")]

use digua_common_db::{db_execute, db_fetch_all, db_fetch_one, db_fetch_optional, DbPool};

fn test_pg_url() -> Option<String> {
    match std::env::var("TEST_PG_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url),
        _ => {
            eprintln!("[iam_pg_roundtrip] TEST_PG_URL not set — skipped");
            None
        }
    }
}

/// 从 `postgres://u:p@h:p/dbname` 里替换库名。
fn with_db(url: &str, db: &str) -> String {
    let (base, _old) = url
        .rsplit_once('/')
        .expect("TEST_PG_URL must contain a /db");
    format!("{base}/{db}")
}

/// 建一个唯一数据库并返回连到它的 `DbPool`（已跑完 `ensure_schema`）。
async fn fresh_pg_db() -> Option<DbPool> {
    let url = test_pg_url()?;
    let db_name = format!(
        "test_iam_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );

    let admin = sqlx::PgPool::connect(&url).await.expect("admin connect pg");
    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin)
        .await
        .expect("create test database");
    admin.close().await;

    let pool = digua_common_db::init_pool_with(&with_db(&url, &db_name), 5)
        .await
        .expect("connect fresh pg db");
    iam_service::db::ensure_schema(&pool)
        .await
        .expect("ensure iam schema on pg");
    Some(pool)
}

/// ensure_schema 必须**幂等**，且在 PG 上把 V001~V008 全部落齐：
/// V001~V003 的 marker 是业务表本身，V004~V008 是哨兵表。
#[tokio::test]
async fn pg_ensure_schema_applies_all_migrations_idempotently() {
    let Some(pool) = fresh_pg_db().await else {
        return;
    };
    // 幂等：二跑三跑都是 no-op。
    iam_service::db::ensure_schema(&pool)
        .await
        .expect("second ensure_schema must be a no-op");
    iam_service::db::ensure_schema(&pool)
        .await
        .expect("third ensure_schema must be a no-op");

    let markers = [
        "iam_users",                 // V001
        "iam_outbox_events_archive", // V002
        "iam_tenants",               // V003
        "iam_migration_v004_applied",
        "iam_migration_v005_applied",
        "iam_migration_v006_applied",
        "iam_migration_v007_applied",
        "iam_migration_v008_applied",
        "iam_migration_v009_applied",
    ];
    for m in markers {
        let (n,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM information_schema.tables \
             WHERE table_schema = current_schema() AND table_name = $1",
        )
        .bind(m)
        .fetch_one(pool.as_postgres().expect("pg pool"))
        .await
        .unwrap();
        assert_eq!(n, 1, "marker table {m} must exist after ensure_schema");
    }

    // V004 的产物抽查：iam_roles 带 tenant_id（uk_tenant_role_code 复合唯一键）。
    let (uk_cnt,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM pg_indexes \
         WHERE schemaname = current_schema() \
           AND tablename = 'iam_roles' AND indexname = 'uk_tenant_role_code'",
    )
    .fetch_one(pool.as_postgres().expect("pg pool"))
    .await
    .unwrap();
    assert!(uk_cnt >= 1, "uk_tenant_role_code index must exist on PG");
}

/// tenant 自增主键：INSERT 走 `RETURNING id`（归属表 #13，PG 无 last_insert_id），
/// 返回 i64；中文名/枚举状态原样往返。
#[tokio::test]
async fn pg_tenant_insert_returning_id_roundtrip() {
    let Some(pool) = fresh_pg_db().await else {
        return;
    };
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO iam_tenants (display_name, slug, status, created_at, updated_at) \
         VALUES ('租户甲', 'tenant-a', 'active', now(), now()) RETURNING id",
    )
    .fetch_one(pool.as_postgres().expect("pg pool"))
    .await
    .unwrap();
    assert!(id > 0, "identity id must be positive i64");

    // 生产读路径：宏查询（`?` 占位符由渲染器改写为 $1）。
    let (slug, status): (String, String) = db_fetch_one!(
        &pool,
        (String, String),
        "SELECT slug, status FROM iam_tenants WHERE id = ?",
        id
    )
    .unwrap();
    assert_eq!(slug, "tenant-a");
    assert_eq!(status, "active");
}

/// 用户/角色中文往返 + bool 列：db_*! 宏读写（email_verified 等 BOOLEAN）。
#[tokio::test]
async fn pg_user_and_role_roundtrip_chinese_bool() {
    let Some(pool) = fresh_pg_db().await else {
        return;
    };
    db_execute!(
        &pool,
        "INSERT INTO iam_tenants (display_name, slug, status, created_at, updated_at) \
         VALUES ('租户乙', 'tenant-b', 'active', now(), now())"
    )
    .unwrap();
    let (tid,): (i64,) = db_fetch_one!(
        &pool,
        (i64,),
        "SELECT id FROM iam_tenants WHERE slug = ?",
        "tenant-b"
    )
    .unwrap();

    db_execute!(
        &pool,
        "INSERT INTO iam_users (id, tenant_id, username, password_hash, salt, nickname, status, email_verified, phone_verified, created_at, updated_at) \
         VALUES ('u-1', ?, '张三', 'hash', 'salt', '张三昵称', 'active', TRUE, FALSE, now(), now())",
        tid
    )
    .unwrap();
    db_execute!(
        &pool,
        "INSERT INTO iam_roles (id, tenant_id, name, code, data_scope, status, created_at, updated_at) \
         VALUES ('r-1', ?, '管理员', 'admin', 'all', 'active', now(), now())",
        tid
    )
    .unwrap();

    let (nickname, email_verified, phone_verified): (String, bool, bool) = db_fetch_one!(
        &pool,
        (String, bool, bool),
        "SELECT nickname, email_verified, phone_verified FROM iam_users WHERE id = ?",
        "u-1"
    )
    .unwrap();
    assert_eq!(nickname, "张三昵称");
    assert!(email_verified, "bool 列必须原样往返");
    assert!(!phone_verified);

    // 软删过滤 + JOIN 读（auth_svc.get_user_roles 同款形态）。
    db_execute!(
        &pool,
        "INSERT INTO iam_user_roles (user_id, role_id, created_at) VALUES (?, ?, now())",
        "u-1",
        "r-1"
    )
    .unwrap();
    let roles: Vec<(String,)> = db_fetch_all!(
        &pool,
        (String,),
        "SELECT r.code FROM iam_roles r INNER JOIN iam_user_roles ur ON r.id = ur.role_id \
         WHERE ur.user_id = ? AND r.status = 'active' AND r.deleted_at IS NULL",
        "u-1"
    )
    .unwrap();
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].0, "admin");
}

/// internal_membership 的成员聚合格式：PG `STRING_AGG(role_id::text, ',')`
/// 必须与 MySQL `GROUP_CONCAT(role_id)` 输出逐字节一致（逗号分隔、无空格、
/// 按 role_id 排序稳定），下游（网关/前端）按此格式解析。
#[tokio::test]
async fn pg_membership_string_agg_matches_mysql_format() {
    let Some(pool) = fresh_pg_db().await else {
        return;
    };
    db_execute!(
        &pool,
        "INSERT INTO iam_tenants (display_name, slug, status, created_at, updated_at) \
         VALUES ('租户丙', 'tenant-c', 'active', now(), now())"
    )
    .unwrap();
    let (tid,): (i64,) = db_fetch_one!(
        &pool,
        (i64,),
        "SELECT id FROM iam_tenants WHERE slug = ?",
        "tenant-c"
    )
    .unwrap();

    db_execute!(
        &pool,
        "INSERT INTO iam_users (id, tenant_id, username, password_hash, salt, nickname, status, created_at, updated_at) \
         VALUES ('u-9', ?, '聚合用户', 'hash', 'salt', '聚合用户', 'active', now(), now())",
        tid
    )
    .unwrap();
    for (rid, rname) in [("r-a", "角色A"), ("r-b", "角色B")] {
        db_execute!(
            &pool,
            "INSERT INTO iam_roles (id, tenant_id, name, code, data_scope, status, created_at, updated_at) \
             VALUES (?, ?, ?, ?, 'self', 'active', now(), now())",
            rid,
            tid,
            rname,
            rid
        )
        .unwrap();
        db_execute!(
            &pool,
            "INSERT INTO iam_user_roles (user_id, role_id, created_at) VALUES (?, ?, now())",
            "u-9",
            rid
        )
        .unwrap();
    }

    // 与 internal_membership 的 PG 臂同款 SQL 形态：STRING_AGG + ORDER BY。
    let (agg,): (Option<String>,) = sqlx::query_as(
        "SELECT STRING_AGG(ur.role_id::text, ',' ORDER BY ur.role_id) \
         FROM iam_user_roles ur WHERE ur.user_id = $1",
    )
    .bind("u-9")
    .fetch_one(pool.as_postgres().expect("pg pool"))
    .await
    .unwrap();
    assert_eq!(
        agg.as_deref(),
        Some("r-a,r-b"),
        "STRING_AGG 输出必须与 MySQL GROUP_CONCAT 格式一致（逗号分隔、无空格）"
    );
}

/// 登录日志与 refresh token 的 PG 往返（login 全链的表侧）：
/// success BOOLEAN、revoked=FALSE 过滤、NOW() 时间戳列。
#[tokio::test]
async fn pg_login_log_and_refresh_token_roundtrip() {
    let Some(pool) = fresh_pg_db().await else {
        return;
    };
    db_execute!(
        &pool,
        "INSERT INTO iam_tenants (display_name, slug, status, created_at, updated_at) \
         VALUES ('租户丁', 'tenant-d', 'active', now(), now())"
    )
    .unwrap();
    let (tid,): (i64,) = db_fetch_one!(
        &pool,
        (i64,),
        "SELECT id FROM iam_tenants WHERE slug = ?",
        "tenant-d"
    )
    .unwrap();
    db_execute!(
        &pool,
        "INSERT INTO iam_users (id, tenant_id, username, password_hash, salt, nickname, status, created_at, updated_at) \
         VALUES ('u-7', ?, '登录用户', 'hash', 'salt', '登录用户', 'active', now(), now())",
        tid
    )
    .unwrap();

    // login log：成功/失败两行，bool 往返。
    db_execute!(
        &pool,
        "INSERT INTO iam_login_logs (id, user_id, username, ip_address, user_agent, login_type, success, error_message, created_at) \
         VALUES ('log-1', 'u-7', '登录用户', '127.0.0.1', 'ua', 'password', FALSE, 'bad_password', now())"
    )
    .unwrap();
    db_execute!(
        &pool,
        "INSERT INTO iam_login_logs (id, user_id, username, ip_address, user_agent, login_type, success, error_message, created_at) \
         VALUES ('log-2', 'u-7', '登录用户', '127.0.0.1', 'ua', 'password', TRUE, NULL, now())"
    )
    .unwrap();
    let fails: Vec<(String,)> = db_fetch_all!(
        &pool,
        (String,),
        "SELECT id FROM iam_login_logs WHERE user_id = ? AND success = FALSE",
        "u-7"
    )
    .unwrap();
    assert_eq!(fails.len(), 1, "success=FALSE 过滤必须命中失败行");
    assert_eq!(fails[0].0, "log-1");

    // refresh token：revoked=FALSE 的活跃会话过滤。
    db_execute!(
        &pool,
        "INSERT INTO iam_refresh_tokens (id, user_id, token_hash, expires_at, client_ip, user_agent, revoked, created_at, updated_at) \
         VALUES ('rt-1', 'u-7', 'hash-1', now() + interval '30 day', '127.0.0.1', 'ua', FALSE, now(), now())"
    )
    .unwrap();
    db_execute!(
        &pool,
        "INSERT INTO iam_refresh_tokens (id, user_id, token_hash, expires_at, client_ip, user_agent, revoked, revoked_at, created_at, updated_at) \
         VALUES ('rt-2', 'u-7', 'hash-2', now() + interval '30 day', '127.0.0.1', 'ua', TRUE, now(), now(), now())"
    )
    .unwrap();
    let active: Option<(String,)> = db_fetch_optional!(
        &pool,
        (String,),
        "SELECT id FROM iam_refresh_tokens WHERE user_id = ? AND revoked = FALSE ORDER BY created_at DESC",
        "u-7"
    )
    .unwrap();
    assert_eq!(active.map(|(id,)| id).as_deref(), Some("rt-1"));
}

/// outbox 表（V001 建）在 PG 侧的 enqueue 语义往返：envelope_json JSONB、
/// pending 行 claim 后 published_at 落 TIMESTAMPTZ(3)。归档任务的 PG 全链
/// （advisory lock / 搬运 / purge）已由 `libs/common-event/tests/outbox_pg_test.rs`
/// 覆盖，这里不重复。
#[tokio::test]
async fn pg_outbox_envelope_jsonb_roundtrip() {
    let Some(pool) = fresh_pg_db().await else {
        return;
    };
    let event_id = uuid::Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "user_id": "u-中文",
        "tenant_id": 1,
        "reason": "pg_roundtrip",
    });
    db_execute!(
        &pool,
        "INSERT INTO iam_outbox_events (event_id, event_type, routing_key, envelope_json, created_at) \
         VALUES (?, 'iam.permission.changed', 'iam.permission.changed', ?, now())",
        &event_id,
        &payload
    )
    .unwrap();

    let (uid,): (String,) = sqlx::query_as(
        "SELECT envelope_json->>'user_id' \
         FROM iam_outbox_events WHERE event_id = $1",
    )
    .bind(&event_id)
    .fetch_one(pool.as_postgres().expect("pg pool"))
    .await
    .unwrap();
    assert_eq!(uid, "u-中文", "JSONB 路径取值 + 中文往返");
}
