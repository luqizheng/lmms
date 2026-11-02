//! config-service 在 **PostgreSQL** 上的读写往返验证。
//!
//! 这是 S3 的核心目标：证明**同一份 SQL 源码**在 PG 上真的能跑通
//! （迁移建表 → 写入 → 读回 → 类型正确解码），而不只是"能编译"。
//!
//! 走 **`TEST_PG_URL` 外部容器模式**（本环境 testcontainers 不可用，
//! `file-service/tests/pg_roundtrip_test.rs` 已用同一路径跑通）。
//! 无 `TEST_PG_URL` 时打印 `[skip]` 并直接返回，不失败。
//!
//! 每个用例建一个**唯一数据库**（PG 的 `CREATE DATABASE`），避免并行测试互相污染。
//! ⚠️ 不能用 `SET search_path` + 连接池——连接池下每条语句可能落到不同连接，
//! search_path 不会保持。
//!
//! 覆盖的三类「PG 专属陷阱」：
//! 1. `u32` 分页参数（sqlx-postgres 无 `u32: Encode` 实现 → 必须 `as i64`）
//! 2. `key` 是保留字（MySQL 反引号 ↔ PG 双引号，走 `escape_identifier`）
//! 3. 多表 UPDATE / DELETE（PG 用 `SET ... FROM` / `DELETE ... USING`，与 MySQL JOIN 写法无交集）
//! 此外还验证 `TIMESTAMPTZ → DateTime<Utc>` 与中文 UTF-8 往返。

#![cfg(feature = "postgres")]

use config_service::models::config_item::{CreateConfigItem, QueryConfigItems, UpdateConfigItem};
use config_service::models::namespace::{CreateNamespace, QueryNamespaces, UpdateNamespace};
use config_service::models::version::QueryVersions;
use config_service::services::ConfigService;
use digua_common_db::DbPool;
use digua_common_jwt::JWKSCache;
use digua_jwt_ctx::TenantScope;
use jwt_simple::prelude::*;
use std::sync::Arc;

fn test_pg_url() -> Option<String> {
    match std::env::var("TEST_PG_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url),
        _ => {
            eprintln!("[config_pg_roundtrip] TEST_PG_URL not set — skipped");
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
async fn fresh_pg_pool() -> Option<DbPool> {
    let url = test_pg_url()?;
    let db_name = format!(
        "test_cfg_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );

    // 用管理连接建库（CREATE DATABASE 不能在事务里，sqlx 默认 autocommit，OK）。
    let admin = sqlx::PgPool::connect(&url).await.expect("admin connect pg");
    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin)
        .await
        .expect("create test database");
    admin.close().await;

    let pool = config_service::db::init_pool(&with_db(&url, &db_name))
        .await
        .expect("init_pool on fresh pg db");
    config_service::db::ensure_schema(&pool)
        .await
        .expect("ensure_schema on pg");
    Some(pool)
}

/// 构造真实 `ConfigService`：PG 池 + 现生成的 JWKS（服务方法本身不需要 JWT 校验）。
fn make_service(pool: DbPool) -> ConfigService {
    let key_pair = RS256KeyPair::generate(2048).expect("rsa keypair");
    let public_pem = key_pair.public_key().to_pem().expect("public pem");
    let jwks = Arc::new(JWKSCache::from_pem(&public_pem).expect("jwks"));
    ConfigService::new(pool, jwks)
}

fn create_ns(app: &str) -> CreateNamespace {
    CreateNamespace {
        app: app.to_string(),
        env: "prod".to_string(),
        profile: Some("默认".to_string()),
        description: Some("配置命名空间".to_string()),
    }
}

fn create_item(namespace_id: &str, key: &str, value: &str) -> CreateConfigItem {
    CreateConfigItem {
        namespace_id: namespace_id.to_string(),
        key: key.to_string(),
        value: value.to_string(),
        value_type: "string".to_string(),
        is_secret: false,
        description: Some("演示配置项".to_string()),
    }
}

// ---------------------------------------------------------------------------
// 迁移层
// ---------------------------------------------------------------------------

/// `ensure_schema` 必须**幂等**：重复调用不得报错、不得重复建表。
#[tokio::test]
async fn pg_ensure_schema_is_idempotent() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };
    config_service::db::ensure_schema(&pool)
        .await
        .expect("second ensure_schema must be a no-op");
    config_service::db::ensure_schema(&pool)
        .await
        .expect("third ensure_schema must be a no-op");
}

/// V003 在 PG 侧是**空操作**，但哨兵表必须存在（marker 语义两侧一致），
/// 且 `cfg_namespaces.tenant_id` 必须是 `bigint`（PG 无 UNSIGNED，天然有符号）。
#[tokio::test]
async fn pg_v003_marker_exists_and_tenant_id_is_bigint() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };
    let pg = pool.as_postgres().expect("pg pool");

    for marker in ["cfg_migration_v002_applied", "cfg_migration_v003_applied"] {
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

    let ty: (String,) = sqlx::query_as(
        "SELECT data_type::text FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = 'cfg_namespaces' \
           AND column_name = 'tenant_id'",
    )
    .fetch_one(pg)
    .await
    .expect("describe tenant_id");
    assert_eq!(ty.0, "bigint", "tenant_id 必须是 BIGINT（有符号）");

    // `key` 列在 PG 侧是带引号的 "key"（保留字），确认迁移按预期创建。
    let key_ty: (String,) = sqlx::query_as(
        "SELECT data_type::text FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = 'cfg_config_items' \
           AND column_name = 'key'",
    )
    .fetch_one(pg)
    .await
    .expect("describe key column");
    assert_eq!(key_ty.0, "character varying");
}

// ---------------------------------------------------------------------------
// 生产代码路径（不是手抄 SQL）
// ---------------------------------------------------------------------------
//
// 下面直接构造 `ConfigService` 并调用生产方法，因此：
// - 生产 SQL 一旦写成 PG 不兼容的形态，这里立刻红；
// - 顺带覆盖 `size as i64` / `offset as i64`（u32 在 PG 上无法 bind 的真实回归）。

/// 命名空间 CRUD：`create_namespace` / `query_namespaces` / `get_namespace` /
/// `update_namespace` / `delete_namespace` 全部走生产代码。
#[tokio::test]
async fn pg_production_namespace_crud() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };
    let service = make_service(pool.clone());

    // 1) create_namespace：raw INSERT（8 binds）+ 中文 profile/description。
    let ns = service
        .create_namespace(create_ns("订单服务"), &TenantScope::Single(7))
        .await
        .expect("create_namespace must work on PG");
    assert_eq!(ns.tenant_id, 7i64, "tenant_id 必须按 i64 存取");
    assert_eq!(ns.app, "订单服务", "多字节 UTF-8 必须原样往返");
    assert_eq!(ns.profile.as_deref(), Some("默认"));
    // TIMESTAMPTZ 必须以 UTC 时刻精确写入（会话时区已锁 UTC）。
    assert!(ns.created_at.timestamp() > 0);

    // 2) get_namespace：`where_id` + `and_tenant`。
    let got = service
        .get_namespace(&ns.id, &TenantScope::Single(7))
        .await
        .expect("get_namespace must work on PG");
    assert_eq!(got.id, ns.id);
    assert_eq!(got.app, "订单服务");

    // 3) query_namespaces：`LIMIT ? OFFSET ?`（size/offset 必须 as i64）。
    let created = service
        .create_namespace(create_ns("支付服务"), &TenantScope::Single(7))
        .await
        .expect("second namespace on PG");
    let page1 = service
        .query_namespaces(
            QueryNamespaces {
                app: None,
                env: None,
                page: Some(1),
                size: Some(1),
            },
            &TenantScope::Single(7),
        )
        .await
        .expect("query_namespaces must work on PG");
    assert_eq!(page1.items.len(), 1, "size=1 分页必须在 PG 上生效");
    assert_eq!(page1.page, 1);
    assert_eq!(page1.size, 1);

    let page2 = service
        .query_namespaces(
            QueryNamespaces {
                app: None,
                env: None,
                page: Some(2),
                size: Some(1),
            },
            &TenantScope::Single(7),
        )
        .await
        .expect("query_namespaces page 2 on PG");
    assert_eq!(page2.items.len(), 1, "OFFSET 分页必须在 PG 上生效");
    assert_ne!(
        page1.items[0].id, page2.items[0].id,
        "第 1 页与第 2 页必须是不同行"
    );

    // 租户隔离在 PG 上必须同样生效。
    let other = service
        .query_namespaces(
            QueryNamespaces {
                app: None,
                env: None,
                page: Some(1),
                size: Some(20),
            },
            &TenantScope::Single(999),
        )
        .await
        .expect("query_namespaces with foreign tenant on PG");
    assert!(
        other.items.is_empty(),
        "跨租户查询必须看不到别的租户的命名空间"
    );

    // 4) update_namespace：单表 UPDATE + where_id + and_tenant。
    let updated = service
        .update_namespace(
            &ns.id,
            UpdateNamespace {
                description: Some("改过的描述".to_string()),
            },
            &TenantScope::Single(7),
        )
        .await
        .expect("update_namespace must work on PG");
    assert_eq!(updated.description.as_deref(), Some("改过的描述"));

    // 5) delete_namespace：DELETE + where_id + and_tenant。
    service
        .delete_namespace(&created.id, &TenantScope::Single(7))
        .await
        .expect("delete_namespace must work on PG");
    assert!(
        service
            .get_namespace(&created.id, &TenantScope::Single(7))
            .await
            .is_err(),
        "删除后必须读不到"
    );
}

/// 配置项 CRUD：覆盖 `key` 保留字转义、多表 UPDATE（PG `SET ... FROM`）、
/// 多表 DELETE（PG `USING`）、`LIKE` 过滤与 `TIMESTAMPTZ` 解码。
#[tokio::test]
async fn pg_production_config_item_crud() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };
    let service = make_service(pool.clone());

    // 基线命名空间（tenant 7）。
    let ns = service
        .create_namespace(create_ns("网关"), &TenantScope::Single(7))
        .await
        .expect("create namespace on PG");

    // 1) create_config_item：INSERT 里 `key` 必须被转义成 "key"（PG 保留字）。
    let item = service
        .create_config_item(
            create_item(&ns.id, "timeout", "超时=30秒"),
            &TenantScope::Single(7),
        )
        .await
        .expect("create_config_item must work on PG");
    assert_eq!(item.key, "timeout");
    assert_eq!(item.value, "超时=30秒", "中文 value 必须原样往返");
    assert!(!item.is_secret);

    // 命名空间不属于本租户时必须拒绝（and_tenant 生效）。
    let denied = service
        .create_config_item(create_item(&ns.id, "k", "v"), &TenantScope::Single(999))
        .await;
    assert!(denied.is_err(), "跨租户创建配置项必须失败");

    // 2) get_config_item：JOIN + i.id = ? （ConfigItem 全部字段解码）。
    let got = service
        .get_config_item(&item.id, &TenantScope::Single(7))
        .await
        .expect("get_config_item must work on PG");
    assert_eq!(got.id, item.id);
    assert_eq!(got.namespace_id, ns.id);
    assert_eq!(got.key, "timeout");
    assert_eq!(got.value, "超时=30秒");
    assert!(!got.is_secret, "is_secret 必须按 bool 解码");
    assert_eq!(
        got.created_at.timestamp(),
        item.created_at.timestamp(),
        "TIMESTAMPTZ 必须精确往返（会话 TZ 已锁 UTC）"
    );

    // 3) query_config_items：`i."key" LIKE ?` + 分页（size/offset as i64）。
    let filtered = service
        .query_config_items(
            QueryConfigItems {
                namespace_id: Some(ns.id.clone()),
                key: Some("time".to_string()),
                page: Some(1),
                size: Some(10),
            },
            &TenantScope::Single(7),
        )
        .await
        .expect("query_config_items with key filter must work on PG");
    assert_eq!(filtered.items.len(), 1, "key LIKE 过滤必须在 PG 上生效");
    assert_eq!(filtered.items[0].id, item.id);

    let paged = service
        .query_config_items(
            QueryConfigItems {
                namespace_id: None,
                key: None,
                page: Some(1),
                size: Some(1),
            },
            &TenantScope::Single(7),
        )
        .await
        .expect("query_config_items paging on PG");
    assert_eq!(paged.items.len(), 1, "LIMIT 必须在 PG 上生效");

    // 4) update_config_item：PG 分支 `UPDATE ... SET ... FROM ...`（COALESCE 保留旧值）。
    let updated = service
        .update_config_item(
            &item.id,
            UpdateConfigItem {
                value: Some("超时=60秒".to_string()),
                value_type: None,
                is_secret: Some(true),
                description: None,
            },
            &TenantScope::Single(7),
        )
        .await
        .expect("update_config_item must work on PG");
    assert_eq!(updated.value, "超时=60秒", "多表 UPDATE 必须真的改到列");
    assert!(updated.is_secret, "is_secret 必须被 COALESCE 更新");
    assert_eq!(
        updated.value_type, "string",
        "未提供的字段必须由 COALESCE 保留旧值"
    );
    assert_eq!(
        updated.description.as_deref(),
        Some("演示配置项"),
        "未提供的 description 必须保留旧值"
    );

    // 跨租户更新必须失败（UPDATE 的 tenant 过滤在 PG 上不能失效）。
    let denied_update = service
        .update_config_item(
            &item.id,
            UpdateConfigItem {
                value: Some("hacked".to_string()),
                value_type: None,
                is_secret: None,
                description: None,
            },
            &TenantScope::Single(999),
        )
        .await;
    assert!(denied_update.is_err(), "跨租户更新必须失败");

    // 5) query_versions：JOIN 查询在 PG 上可跑（此时无版本记录）。
    let versions = service
        .query_versions(
            QueryVersions {
                namespace_id: Some(ns.id.clone()),
                item_id: None,
                page: Some(1),
                size: Some(10),
            },
            &TenantScope::Single(7),
        )
        .await
        .expect("query_versions must work on PG");
    assert!(versions.items.is_empty(), "尚未产生版本记录");

    // 6) delete_config_item：PG 分支 `DELETE ... USING ...`。
    service
        .delete_config_item(&item.id, &TenantScope::Single(7))
        .await
        .expect("delete_config_item must work on PG");
    assert!(
        service
            .get_config_item(&item.id, &TenantScope::Single(7))
            .await
            .is_err(),
        "删除后必须读不到"
    );
}

/// 跨租户（`CrossTenant`）路径在 PG 上同样不能多出 tenant 过滤。
#[tokio::test]
async fn pg_cross_tenant_scope_sees_all_tenants() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };
    let service = make_service(pool.clone());

    service
        .create_namespace(create_ns("租户一"), &TenantScope::Single(7))
        .await
        .expect("create tenant 7 namespace");
    service
        .create_namespace(create_ns("租户二"), &TenantScope::Single(8))
        .await
        .expect("create tenant 8 namespace");

    let all = service
        .query_namespaces(
            QueryNamespaces {
                app: None,
                env: None,
                page: Some(1),
                size: Some(20),
            },
            &TenantScope::CrossTenant,
        )
        .await
        .expect("cross tenant query on PG");
    assert_eq!(all.items.len(), 2, "跨租户应看到 2 个命名空间");
    let tenants: Vec<i64> = all.items.iter().map(|n| n.tenant_id).collect();
    assert!(tenants.contains(&7i64) && tenants.contains(&8i64));
}
