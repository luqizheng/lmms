//! prue_backend 在 **PostgreSQL** 上的读写往返验证（三方言服务的第三臂）。
//!
//! `prue_backend` 默认编译 mysql+sqlite，PostgreSQL 是可选方言
//! （`--features postgres`）。本文件证明**同一份 SQL 源码**在 PG 上真的能跑通：
//! 迁移建表 → 写入 → 读回 → 类型正确解码，并且覆盖本服务把 9 对
//! `DbPool::Mysql/Sqlite` match 收敛成单份 SQL 后的三条方言相关路径：
//!
//! 1. `routes_json::text`（PG 的 json 列不能直接解到 `String`）
//! 2. `now()`（`TIMESTAMPTZ` 列，会话时区由 `init_pool` 锁 UTC）
//! 3. `is_active` / `deleted_at` 的布尔与时间语义
//!
//! 走 **`TEST_PG_URL` 外部容器模式**（本环境 testcontainers 不可用）。
//! 无该变量时打印 `[skip]` 并直接返回，不失败。
//!
//! 每次跑建一个**唯一数据库**，避免并行测试互相污染。

#![cfg(feature = "postgres")]

use chrono::Utc;
use digua_common_jwt::{JWKSCache, JwksConfig};
use prue_backend::{
    db,
    dto::{CreateMenuConfigRequest, UpdateMenuConfigRequest},
    service::MenuConfigService,
    state::AppState,
};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

fn test_pg_url() -> Option<String> {
    match std::env::var("TEST_PG_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url),
        _ => {
            eprintln!("[prue_pg_roundtrip] TEST_PG_URL not set — skipped");
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

/// 建一个唯一数据库并返回连到它的 `DbPool`（**未**跑 `ensure_schema`）。
async fn fresh_pg_pool() -> Option<digua_common_db::DbPool> {
    let url = test_pg_url()?;
    let db_name = format!("test_prue_{}_{}", std::process::id(), nanoid::nanoid!());

    // 用管理连接建库（CREATE DATABASE 不能在事务里，sqlx 默认 autocommit，OK）。
    let admin = sqlx::PgPool::connect(&url).await.expect("admin connect pg");
    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin)
        .await
        .expect("create test database");
    admin.close().await;

    // 与 main.rs 同一条建池路径（PG 分支会在 after_connect 里锁 UTC 会话时区）。
    Some(
        digua_common_db::init_pool(&with_db(&url, &db_name))
            .await
            .expect("init_pool on fresh pg db"),
    )
}

/// 建库 + 跑 `ensure_schema`，返回可用的 `MenuConfigService`。
async fn fresh_service() -> Option<MenuConfigService> {
    let pool = fresh_pg_pool().await?;
    db::ensure_schema(&pool).await.expect("ensure_schema on pg");
    Some(service_from_pool(pool))
}

fn service_from_pool(pool: digua_common_db::DbPool) -> MenuConfigService {
    let jwks = Arc::new(JWKSCache::new(
        JwksConfig {
            url: "http://localhost:0/.well-known/jwks.json".into(),
            refresh_interval: Duration::from_secs(3600),
            request_timeout: Duration::from_secs(5),
        },
        reqwest::Client::new(),
    ));
    MenuConfigService::new(AppState::new(pool, jwks))
}

fn sample_routes() -> Value {
    json!([
        {
            "path": "/系统管理",
            "name": "System",
            "meta": { "title": "系统管理", "_menu": { "id": 1, "menuType": 0 } },
            "children": [
                {
                    "path": "/系统管理/菜单",
                    "name": "SystemMenu",
                    "meta": { "title": "菜单", "_menu": { "id": 2, "menuType": 1 } }
                }
            ]
        }
    ])
}

fn create_req(name: &str) -> CreateMenuConfigRequest {
    CreateMenuConfigRequest {
        name: name.to_string(),
        description: Some("中文描述".to_string()),
        routes_json: sample_routes(),
        created_by: Some("admin".to_string()),
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
    db::ensure_schema(&pool).await.expect("first ensure_schema");
    db::ensure_schema(&pool)
        .await
        .expect("second ensure_schema must be a no-op");
    db::ensure_schema(&pool)
        .await
        .expect("third ensure_schema must be a no-op");
}

// ---------------------------------------------------------------------------
// 生产代码路径（不是手抄 SQL）
// ---------------------------------------------------------------------------
//
// 下面全部调用 `MenuConfigService` 的**生产方法**，因此菜单配置服务里
// 收敛之后的单份 SQL 一旦写成 PG 不兼容的形态，这里立刻红。

/// 核心用例：create（draft）→ list → publish → active_routes，全程生产方法。
///
/// 断言点对应 PG 的实质差异：
/// - `list()` / `get()` 依赖 `routes_json::text` → `String` 解码
/// - `created_at` / `updated_at` 是 `DateTime<Utc>`（`TIMESTAMPTZ` 解码）
/// - `is_active` 是真正的 `bool`（PG `BOOLEAN`，不是 TINYINT）
/// - `publish()` 的两条 UPDATE 在事务里生效，且新项成为唯一 active
#[tokio::test]
async fn pg_production_create_publish_active() {
    let Some(svc) = fresh_service().await else {
        return;
    };

    // 1) create：INSERT ... is_active 绑 bool + now() 写 TIMESTAMPTZ。
    let detail = svc
        .create(create_req("主菜单"))
        .await
        .expect("create must work on PG");
    assert_eq!(detail.version, 1);
    assert_eq!(detail.name, "主菜单", "中文必须原样往返");
    assert_eq!(detail.status, "draft");
    assert!(!detail.is_active, "新建项必须是未激活");
    assert_eq!(
        detail.routes_json,
        sample_routes(),
        "routes_json 必须完整往返"
    );
    assert_eq!(detail.routes_etag.len(), 64, "etag 必须是 64 位 hex");
    assert_eq!(detail.description.as_deref(), Some("中文描述"));
    assert_eq!(detail.created_by.as_deref(), Some("admin"));
    assert_eq!(detail.published_at, None);

    // created_at 是 DateTime<Utc>：能解出且是「刚刚」（±10 分钟），
    // 证明 TIMESTAMPTZ 解码正确、会话时区不是本地时区导致偏移。
    let now = Utc::now();
    let skew = (now - detail.created_at).num_seconds().abs();
    assert!(
        skew < 600,
        "created_at 必须接近当前 UTC 时间（差 {skew}s），实际 {}",
        detail.created_at
    );
    assert_eq!(
        detail.created_at.timestamp(),
        detail.updated_at.timestamp(),
        "新建时 created_at 与 updated_at 应一致"
    );

    // 2) list：SELECT fields（含 routes_json::text）→ MenuConfigRow 解码。
    let page = svc.list().await.expect("list must work on PG");
    assert_eq!(page.total, 1, "列表应看到 1 条");
    assert_eq!(page.items[0].id, detail.id);
    assert_eq!(page.items[0].version, 1);
    assert!(!page.items[0].is_active);

    // 3) publish：事务内「全量置 false」+「激活目标」。
    let published = svc.publish(&detail.id).await.expect("publish on PG");
    assert_eq!(published.status, "published");
    assert!(
        published.is_active,
        "发布后 is_active 必须为 true（PG BOOLEAN）"
    );
    let published_at = published.published_at.expect("published_at 必须有值");
    assert!((Utc::now() - published_at).num_seconds().abs() < 600);

    // 4) active_routes：单字段查询走 `routes_json::text`，并递归剥掉 `_menu`。
    let active = svc.active_routes().await.expect("active_routes on PG");
    assert!(active.is_array(), "active_routes 必须返回数组");
    let serialized = serde_json::to_string(&active).unwrap();
    assert!(
        !serialized.contains("_menu"),
        "active_routes 必须剥掉 _menu：{serialized}"
    );
    assert!(serialized.contains("系统管理"), "中文路由必须原样返回");

    // detail 侧仍保留 _menu（管理 UI 需要）。
    let still_detail = svc.get(&detail.id).await.expect("get on PG");
    assert!(serde_json::to_string(&still_detail.routes_json)
        .unwrap()
        .contains("_menu"));
}

/// 并发发布语义：第二次 publish 必须让前一个不再 active（只有一个是 active）。
#[tokio::test]
async fn pg_publish_deactivates_previous() {
    let Some(svc) = fresh_service().await else {
        return;
    };

    let first = svc.create(create_req("菜单一")).await.expect("create 1");
    // 第二个配置用**不同**的路由内容，这样才能断言 active_routes 真的切过去了。
    let second = svc
        .create(CreateMenuConfigRequest {
            name: "菜单二".to_string(),
            description: None,
            routes_json: json!([{ "path": "/second-only", "name": "Second", "meta": { "title": "第二套" } }]),
            created_by: None,
        })
        .await
        .expect("create 2");
    assert_eq!(second.version, 2, "version 必须递增");

    svc.publish(&first.id).await.expect("publish 1");
    assert!(svc.get(&first.id).await.unwrap().is_active);

    svc.publish(&second.id).await.expect("publish 2");
    assert!(
        !svc.get(&first.id).await.unwrap().is_active,
        "发布第二个后，第一个必须被置为未激活（事务内 UPDATE 生效）"
    );
    assert!(svc.get(&second.id).await.unwrap().is_active);

    // active_routes 只应返回最新发布的那个（ORDER BY version DESC LIMIT 1）。
    let active = svc.active_routes().await.expect("active_routes");
    let serialized = serde_json::to_string(&active).unwrap();
    assert!(
        serialized.contains("/second-only"),
        "应返回最新发布的配置，实际：{serialized}"
    );
}

/// update（etag 乐观锁）/ clone / delete（软删）在 PG 上同样成立。
#[tokio::test]
async fn pg_production_update_clone_delete() {
    let Some(svc) = fresh_service().await else {
        return;
    };

    let detail = svc.create(create_req("原始")).await.expect("create");

    // 1) update：命中 etag → 轮换 etag + 写 updated_at = now()。
    let new_routes =
        json!([{ "path": "/dashboard", "name": "Dashboard", "meta": { "title": "首页" } }]);
    let updated = svc
        .update(
            &detail.id,
            UpdateMenuConfigRequest {
                name: Some("改名".to_string()),
                description: None,
                routes_json: Some(new_routes.clone()),
                if_match_etag: Some(detail.routes_etag.clone()),
            },
        )
        .await
        .expect("update must work on PG");
    assert_eq!(updated.name, "改名");
    assert_eq!(updated.routes_json, new_routes);
    assert_ne!(updated.routes_etag, detail.routes_etag, "etag 必须轮换");
    assert!(
        updated.updated_at.timestamp() >= detail.updated_at.timestamp(),
        "updated_at 必须前进或持平"
    );

    // 2) 陈旧 etag → EtagConflict（不是 DB 错误）。
    let stale = svc
        .update(
            &detail.id,
            UpdateMenuConfigRequest {
                name: None,
                description: None,
                routes_json: None,
                if_match_etag: Some("stale-etag".to_string()),
            },
        )
        .await;
    assert!(stale.is_err(), "陈旧 etag 必须失败");

    // 3) clone：读旧行（含 routes_json::text）→ 插入新行，version 递增。
    let cloned = svc.clone_config(&detail.id).await.expect("clone on PG");
    assert!(cloned.version > detail.version, "clone 必须拿新版本号");
    assert_eq!(cloned.name, "改名 copy");
    assert_eq!(cloned.status, "draft");
    assert_eq!(cloned.routes_json, new_routes);

    // 4) delete：软删（draft 才可删），之后 get 必须 404。
    svc.delete(&cloned.id).await.expect("delete on PG");
    assert!(svc.get(&cloned.id).await.is_err(), "软删后必须读不到");
    // 软删后 version 不得回退（UNIQUE(version) 不能被复用）。
    let after = svc.create(create_req("再来一个")).await.expect("create");
    assert!(
        after.version > cloned.version,
        "软删的行仍占用版本号，新版本必须更大"
    );
}
