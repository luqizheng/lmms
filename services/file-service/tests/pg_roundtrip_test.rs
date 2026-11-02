//! file-service 在 **PostgreSQL** 上的读写往返验证。
//!
//! 这是 S3 的核心目标：证明**同一份 SQL 源码**在 PG 上真的能跑通
//! （迁移建表 → 写入 → 读回 → 类型正确解码），而不只是"能编译"。
//!
//! 走 **`TEST_PG_URL` 外部容器模式**（本环境 testcontainers 不可用，
//! `outbox_pg_test` / `pg_migration_test` 已用同一路径跑通）。
//! 无 `TEST_PG_URL` 时打印 `[skip]` 并直接返回，不失败。
//!
//! 与 MySQL fixture 一致：每次跑建一个**唯一数据库**（PG 的 `CREATE DATABASE`），
//! 避免并行测试互相污染。⚠️ 不能用 `SET search_path` + 连接池——连接池下每条语句
//! 可能落到不同连接，search_path 不会保持（这是 `pg_migration_test` 首版踩过的坑）。

#![cfg(feature = "postgres")]

use chrono::{DateTime, TimeZone, Utc};
use digua_common_db::{db_execute, db_qb_fetch_all, DbPool};
use file_service::models::file_info::FileInfo;

fn test_pg_url() -> Option<String> {
    match std::env::var("TEST_PG_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url),
        _ => {
            eprintln!("[file_pg_roundtrip] TEST_PG_URL not set — skipped");
            None
        }
    }
}

/// 从 `postgres://u:p@h:p/dbname` 里替换库名。
fn with_db(url: &str, db: &str) -> String {
    let (base, _old) = url.rsplit_once('/').expect("TEST_PG_URL must contain a /db");
    format!("{base}/{db}")
}

/// 建一个唯一数据库并返回连到它的 `DbPool`（已跑完 ensure_schema）。
async fn fresh_pg_pool() -> Option<DbPool> {
    let url = test_pg_url()?;
    let db_name = format!(
        "test_file_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );

    // 用管理连接建库（CREATE DATABASE 不能在事务里，sqlx 默认 autocommit，OK）。
    let admin = sqlx::PgPool::connect(&url).await.expect("admin connect pg");
    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin)
        .await
        .expect("create test database");

    let pool = file_service::db::init_pool(&with_db(&url, &db_name))
        .await
        .expect("init_pool on fresh pg db");
    file_service::db::ensure_schema(&pool)
        .await
        .expect("ensure_schema on pg");
    Some(pool)
}

fn sample_file(tenant_id: i64) -> FileInfo {
    let ts: DateTime<Utc> = Utc.with_ymd_and_hms(2026, 11, 2, 12, 0, 0).unwrap();
    FileInfo {
        id: format!("f-{tenant_id}"),
        filename: "报告.txt".to_string(),
        content_type: Some("text/plain".to_string()),
        size: 12,
        hash: Some("deadbeef".to_string()),
        storage_path: "/data/报告.txt".to_string(),
        tenant_id,
        user_id: Some("u-1".to_string()),
        is_temporary: false,
        expires_at: None,
        created_at: ts,
        updated_at: ts,
        url: String::new(),
    }
}

/// 核心用例：迁移 → 写入 → 读回 → **类型逐一校验**。
///
/// 校验点（每条都对应一个已知的 PG 陷阱）：
/// - `tenant_id: i64`  ↔ PG `BIGINT`（§3 #19/#23；u64 在 PG 上无法解码）
/// - `is_temporary: bool` ↔ PG `BOOLEAN`（§3 #12；不是 TINYINT）
/// - `created_at: DateTime<Utc>` ↔ PG `TIMESTAMPTZ`（§3 #3；写错成 TIMESTAMP 会解码失败）
/// - 中文内容（迁移/数据里的多字节 UTF-8 不得被破坏）
#[tokio::test]
async fn pg_file_roundtrip_preserves_types_and_utf8() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };

    let file = sample_file(7);

    // 写入：与 file-service 生产代码 save_file_info **完全相同的 SQL 与 bind 顺序**。
    db_execute!(
        &pool,
        r#"INSERT INTO file_files
           (id, filename, content_type, size, hash, storage_path, tenant_id, user_id, is_temporary, expires_at, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        &file.id,
        &file.filename,
        &file.content_type,
        file.size,
        &file.hash,
        &file.storage_path,
        file.tenant_id,
        &file.user_id,
        file.is_temporary,
        file.expires_at,
        file.created_at,
        file.updated_at
    )
    .expect("INSERT on pg");

    // 读回：走 QueryBuilder（与 query_files 同一条路径，push_bind 自动产出 $n）。
    let rows: Vec<FileInfo> = db_qb_fetch_all!(&pool, FileInfo, |qb| {
        qb.push("SELECT * FROM file_files WHERE id = ").push_bind(&file.id);
    })
    .expect("SELECT on pg");

    assert_eq!(rows.len(), 1, "应读回 1 行");
    let got = &rows[0];

    // 类型与取值逐一核对
    assert_eq!(got.id, file.id);
    assert_eq!(got.tenant_id, 7i64, "tenant_id 必须是 i64（PG BIGINT）");
    assert_eq!(got.filename, "报告.txt", "多字节 UTF-8 必须原样往返");
    assert_eq!(got.storage_path, "/data/报告.txt");
    assert!(!got.is_temporary, "is_temporary 必须是 bool（PG BOOLEAN）");
    assert_eq!(got.size, 12i64);
    // TIMESTAMPTZ → DateTime<Utc>：能解码且时刻精确（秒级）
    assert_eq!(
        got.created_at.timestamp(),
        file.created_at.timestamp(),
        "TIMESTAMPTZ 必须精确往返（会话 TZ 已被 after_connect 锁定为 UTC）"
    );
    assert_eq!(got.updated_at.timestamp(), file.updated_at.timestamp());
    assert_eq!(got.expires_at, None);
}

/// V003 在 PG 侧是**空操作**，但哨兵表必须存在（marker 语义两侧一致）。
#[tokio::test]
async fn pg_v003_is_noop_but_marker_exists() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };
    let pg = pool.as_postgres().expect("pg pool");

    for marker in [
        "file_migration_v002_applied",
        "file_migration_v003_applied",
    ] {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM information_schema.tables \
             WHERE table_schema = current_schema() AND table_name = $1",
        )
        .bind(marker)
        .fetch_one(pg)
        .await
        .expect("probe marker");
        assert_eq!(row.0, 1, "哨兵表 {marker} 必须存在");
    }

    // tenant_id 在 PG 侧本来就是有符号 BIGINT，V003（MySQL 的 MODIFY）无需动作。
    let ty: (String,) = sqlx::query_as(
        "SELECT data_type::text FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = 'file_files' \
           AND column_name = 'tenant_id'",
    )
    .fetch_one(pg)
    .await
    .expect("describe tenant_id");
    assert_eq!(ty.0, "bigint");
}

/// `ensure_schema` 必须**幂等**：重复调用不得报错、不得重复建表。
#[tokio::test]
async fn pg_ensure_schema_is_idempotent() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };
    file_service::db::ensure_schema(&pool)
        .await
        .expect("second ensure_schema must be a no-op");
    file_service::db::ensure_schema(&pool)
        .await
        .expect("third ensure_schema must be a no-op");
}
