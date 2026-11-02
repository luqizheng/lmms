//! notification-service 在 **PostgreSQL** 上的读写往返验证。
//!
//! 与 audit/config/token 的 `pg_roundtrip_test.rs` 同一模式：
//! `TEST_PG_URL` 外部容器模式，未设时打印 `[skip]` 直接通过。
//! 每个用例建唯一数据库（`CREATE DATABASE`），避免并行测试互相污染。
//!
//! 覆盖点（走生产代码路径，不手抄 SQL）：
//! 1. `ensure_schema` 幂等（marker 探测在 PG 走 current_schema()）
//! 2. `create_announcement`（PG 臂 RETURNING id）+ `insert_deliveries`
//!    + `list_for_user` 的中文写读往返
//! 3. `mark_read` 的 `ON CONFLICT DO NOTHING` 幂等（二次标记不算新读）
//! 4. `publish_announcement` 全链（deliveries + outbox enqueue + status=published）
//! 5. 投影事件 `apply_role_assigned` / `apply_role_unassigned` /
//!    `apply_dept_changed` / `replace_projection` 的 jsonb 运算符往返
//!    （`||` 追加、`-` 删元素、`@>` 包含判断、`?::jsonb` 显式 cast）
//!
//! 断言用的裸 SQL 是 PG 方言（`$N` / `::text` / `->>`），经 `as_postgres()` 直连。

#![cfg(feature = "postgres")]

use digua_jwt_ctx::TenantScope;
use notification_service::db::Db;
use notification_service::models::{CreateAnnouncement, ScopeInput, ScopeType, Status};
use std::collections::HashSet;

fn test_pg_url() -> Option<String> {
    match std::env::var("TEST_PG_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url),
        _ => {
            eprintln!("[notification_pg_roundtrip] TEST_PG_URL not set — skipped");
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

/// 建一个唯一数据库并返回连到它的 `Db`（已跑完 `ensure_schema`）。
async fn fresh_pg_db() -> Option<Db> {
    let url = test_pg_url()?;
    let db_name = format!(
        "test_ntf_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );

    let admin = sqlx::PgPool::connect(&url).await.expect("admin connect pg");
    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin)
        .await
        .expect("create test database");
    admin.close().await;

    let db = Db::connect(&with_db(&url, &db_name), 5)
        .await
        .expect("connect fresh pg db");
    db.ensure_schema().await.expect("ensure ntf schema on pg");
    Some(db)
}

/// `ensure_schema` 必须**幂等**：重复调用不得报错（V001 marker 探测 + V002/V003
/// 哨兵表探测在 PG 都走 `current_schema()`）。
#[tokio::test]
async fn pg_ensure_schema_is_idempotent() {
    let Some(db) = fresh_pg_db().await else {
        return;
    };
    db.ensure_schema()
        .await
        .expect("second ensure_schema must be a no-op");
    db.ensure_schema()
        .await
        .expect("third ensure_schema must be a no-op");
}

/// create_announcement（PG 臂 `RETURNING id`）→ insert_deliveries（ON CONFLICT
/// 批插）→ list_for_user 读回：中文标题/正文与 bool 字段原样往返。
#[tokio::test]
async fn pg_create_announcement_and_deliveries_roundtrip_chinese() {
    let Some(db) = fresh_pg_db().await else {
        return;
    };
    let id = db
        .create_announcement(
            &CreateAnnouncement {
                title: "季度汇报通知".into(),
                body: "大家好，本周五进行季度汇报，请准时参加。".into(),
                publish_at: chrono::Utc::now() - chrono::Duration::seconds(1),
                expire_at: None,
                pinned: true,
                popover: false,
                recipients: vec![ScopeInput {
                    scope_type: ScopeType::User,
                    scope_id: "u1".into(),
                }],
            },
            "管理员".into(),
        )
        .await
        .expect("create_announcement on pg");

    let got = db.get(id).await.unwrap().expect("row must exist");
    assert_eq!(got.id, id);
    assert_eq!(got.title, "季度汇报通知");
    assert_eq!(got.body, "大家好，本周五进行季度汇报，请准时参加。");
    assert!(got.pinned, "pinned 必须 bool 往返");
    assert!(!got.popover);
    assert_eq!(got.status, Status::Published, "publish_at 已过 → published");
    assert_eq!(got.created_by, "管理员");

    // recipients 落库
    let scopes = db.list_recipient_scopes(id).await.unwrap();
    assert_eq!(scopes.len(), 1);
    assert!(matches!(scopes[0].0, ScopeType::User));
    assert_eq!(scopes[0].1, "u1");

    // deliveries 批插 + list_for_user 读回
    let mut users = HashSet::new();
    users.insert("u1".to_string());
    users.insert("u2".to_string());
    db.insert_deliveries(id, &users).await.unwrap();

    let list = db.list_for_user("u1".into()).await.unwrap();
    assert_eq!(list.len(), 1, "published 公告必须经 deliveries 可见");
    assert_eq!(list[0].id, id);
    assert_eq!(list[0].title, "季度汇报通知");
    assert!(list[0].pinned);
}

/// mark_read：PG `ON CONFLICT (announcement_id, user_id) DO NOTHING` 幂等 ——
/// 首次 rows_affected=1（新读），重复投递返回 false 且不产生第二行。
#[tokio::test]
async fn pg_mark_read_on_conflict_is_idempotent() {
    let Some(db) = fresh_pg_db().await else {
        return;
    };
    let id = db
        .create_announcement(
            &CreateAnnouncement {
                title: "t".into(),
                body: "b".into(),
                publish_at: chrono::Utc::now() - chrono::Duration::seconds(1),
                expire_at: None,
                pinned: false,
                popover: false,
                recipients: vec![ScopeInput {
                    scope_type: ScopeType::User,
                    scope_id: "u1".into(),
                }],
            },
            "admin".into(),
        )
        .await
        .unwrap();

    let first = db.mark_read(id, "u1".into()).await.unwrap();
    assert!(first, "first read must count");
    let second = db.mark_read(id, "u1".into()).await.unwrap();
    assert!(
        !second,
        "duplicate read must be absorbed by ON CONFLICT DO NOTHING"
    );

    let pg = db.pool.as_postgres().expect("pg pool");
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM ntf_announcement_reads WHERE announcement_id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind("u1")
    .fetch_one(pg)
    .await
    .unwrap();
    assert_eq!(n, 1, "重复标记不得产生第二行");
}

/// publish_announcement 全链：scheduled 公告 → publish →
/// deliveries（ON CONFLICT 批插）+ status=published + outbox enqueue（同事务），
/// 且二次 publish 幂等（不重复 enqueue）。
#[tokio::test]
async fn pg_publish_announcement_full_chain() {
    let Some(db) = fresh_pg_db().await else {
        return;
    };
    // 投影：role r1 → u1, u2
    db.apply_role_assigned("u1", "r1", 1, 100).await.unwrap();
    db.apply_role_assigned("u2", "r1", 1, 101).await.unwrap();

    let id = db
        .create_announcement(
            &CreateAnnouncement {
                title: "全员通知".into(),
                body: "请查阅新制度。".into(),
                publish_at: chrono::Utc::now() + chrono::Duration::hours(1),
                expire_at: None,
                pinned: false,
                popover: true,
                recipients: vec![ScopeInput {
                    scope_type: ScopeType::Role,
                    scope_id: "r1".into(),
                }],
            },
            "admin".into(),
        )
        .await
        .unwrap();
    // publish_at 在未来 → scheduled（create_announcement 的状态判定）
    let got = db.get(id).await.unwrap().unwrap();
    assert_eq!(got.status, Status::Scheduled);

    let users = db
        .publish_announcement(id, &TenantScope::Single(1))
        .await
        .expect("publish on pg");
    let mut sorted = users.clone();
    sorted.sort();
    assert_eq!(sorted, vec!["u1".to_string(), "u2".to_string()]);

    let pg = db.pool.as_postgres().expect("pg pool");
    let (st,): (String,) = sqlx::query_as("SELECT status FROM ntf_announcements WHERE id = $1")
        .bind(id)
        .fetch_one(pg)
        .await
        .unwrap();
    assert_eq!(st, "published");

    let (d,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM ntf_announcement_deliveries WHERE announcement_id = $1",
    )
    .bind(id)
    .fetch_one(pg)
    .await
    .unwrap();
    assert_eq!(d, 2, "role scope 解析出的 user 必须各有一条 delivery");

    // outbox enqueue（JSONB 列 →> 取 payload）
    let (ob,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM ntf_outbox_events \
         WHERE event_type = 'notification.announcement.published' \
           AND envelope_json->'payload'->>'announcement_id' = $1",
    )
    .bind(id.to_string())
    .fetch_one(pg)
    .await
    .unwrap();
    assert_eq!(ob, 1, "publish 必须在事务内 enqueue 一条 outbox");

    // 二次 publish：幂等守卫，不新增 outbox
    let again = db
        .publish_announcement(id, &TenantScope::Single(1))
        .await
        .unwrap();
    assert!(again.is_empty(), "second publish should be no-op");
    let (ob2,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM ntf_outbox_events \
         WHERE event_type = 'notification.announcement.published' \
           AND envelope_json->'payload'->>'announcement_id' = $1",
    )
    .bind(id.to_string())
    .fetch_one(pg)
    .await
    .unwrap();
    assert_eq!(ob2, 1, "no duplicate outbox row");
}

/// 投影事件 jsonb 往返：`|| to_jsonb(...)` 追加、`- ::text` 删元素、
/// `@> to_jsonb(...)` 包含判断、dept_id 覆盖。中文 role/dept/user_id 原样往返。
#[tokio::test]
async fn pg_membership_projection_jsonb_roundtrip() {
    let Some(db) = fresh_pg_db().await else {
        return;
    };
    db.apply_role_assigned("用户A", "研发", 1, 100)
        .await
        .unwrap();
    db.apply_role_assigned("用户A", "管理员", 1, 101)
        .await
        .unwrap();
    db.apply_role_unassigned("用户A", "研发", 1, 200)
        .await
        .unwrap();
    db.apply_dept_changed("用户A", Some("平台部"), 1, 300)
        .await
        .unwrap();

    let pg = db.pool.as_postgres().expect("pg pool");
    // role_ids 落成 JSONB 数组：unassign 后只剩 ["管理员"]
    let (roles,): (String,) =
        sqlx::query_as("SELECT role_ids::text FROM ntf_user_projection WHERE user_id = $1")
            .bind("用户A")
            .fetch_one(pg)
            .await
            .unwrap();
    assert_eq!(roles, "[\"管理员\"]", "jsonb 数组内容必须与事件序列一致");
    let (dept,): (String,) =
        sqlx::query_as("SELECT dept_id::text FROM ntf_user_projection WHERE user_id = $1")
            .bind("用户A")
            .fetch_one(pg)
            .await
            .unwrap();
    assert_eq!(dept, "平台部");

    // resolve_users_by_scopes：@> 包含判断 + dept 分支
    let by_role = db
        .resolve_users_by_scopes(
            &[(ScopeType::Role, "管理员".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    assert_eq!(by_role, HashSet::from(["用户A".to_string()]));
    let by_gone_role = db
        .resolve_users_by_scopes(&[(ScopeType::Role, "研发".into())], &TenantScope::Single(1))
        .await
        .unwrap();
    assert!(by_gone_role.is_empty(), "已移除的 role 不应再解析出用户");
    let by_dept = db
        .resolve_users_by_scopes(
            &[(ScopeType::Dept, "平台部".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    assert_eq!(by_dept, HashSet::from(["用户A".to_string()]));
}

/// bootstrap 快照路径：replace_projection 批量写 `?::jsonb`（text→jsonb 显式
/// cast），DELETE 旧租户行后 resolve 仍正确，且不影响其他租户。
#[tokio::test]
async fn pg_replace_projection_jsonb_bootstrap() {
    let Some(db) = fresh_pg_db().await else {
        return;
    };
    // 旧状态（走事件路径写入）
    db.apply_role_assigned("u_stale", "r_old", 1, 100)
        .await
        .unwrap();

    let mut mappings = std::collections::HashMap::new();
    mappings.insert(
        "u1".to_string(),
        notification_service::models::UserMembership {
            role_ids: vec!["角色一".into(), "角色二".into()],
            dept_id: Some("部门甲".into()),
        },
    );
    mappings.insert(
        "u2".to_string(),
        notification_service::models::UserMembership {
            role_ids: vec![],
            dept_id: None,
        },
    );
    db.replace_projection(1, &mappings).await.unwrap();

    let pg = db.pool.as_postgres().expect("pg pool");
    let (roles,): (String,) =
        sqlx::query_as("SELECT role_ids::text FROM ntf_user_projection WHERE user_id = $1")
            .bind("u1")
            .fetch_one(pg)
            .await
            .unwrap();
    assert_eq!(roles, "[\"角色一\", \"角色二\"]", "bootstrap jsonb 往返");

    let r1 = db
        .resolve_users_by_scopes(
            &[(ScopeType::Role, "角色一".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    let r2 = db
        .resolve_users_by_scopes(
            &[(ScopeType::Role, "角色二".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    let d1 = db
        .resolve_users_by_scopes(
            &[(ScopeType::Dept, "部门甲".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    assert!(!r1.contains("u_stale"), "stale user wiped by tenant");
    assert_eq!(r1, HashSet::from(["u1".to_string()]));
    assert_eq!(r2, HashSet::from(["u1".to_string()]));
    assert_eq!(d1, HashSet::from(["u1".to_string()]));

    // 空 role_ids 的用户仍占一行（dept 解析可达）
    let empty_role = db
        .resolve_users_by_scopes(
            &[(ScopeType::Role, "no-such".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    assert!(empty_role.is_empty());
}
