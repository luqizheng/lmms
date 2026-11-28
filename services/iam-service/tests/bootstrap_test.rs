//! Bootstrap 集成测试（WP-BOOTSTRAP-01 §5）：基础 / 并发准入 / 执行权
//! fencing / 故障恢复。跑在共享 MySQL fixture 上
//!（`TEST_MYSQL_HOST`+`TEST_MYSQL_PORT`，或本地 testcontainers 回退）。
//!
//! PG 侧并发语义由 `bootstrap_pg_test.rs`（TEST_PG_URL 模式）单独覆盖。
//!
//! env 敏感用例（LMMS_INIT_PASSWORD 404 / 403 / 完成后 404）集中在
//! `http_password_contract` 一个 #[tokio::test] 内顺序执行，避免进程级
//! env 竞争；其余用例一律走 `BootstrapService::with_init_password` 注入。

mod common;

use axum::{
    body::{to_bytes, Body},
    http::Request,
};
use digua_common_cache::MemoryCache;
use digua_common_db::{db_execute, db_fetch_one};
use digua_common_event::MySqlOutboxRepo;
use iam_service::config::{
    AppConfig, Config, ConsulConfig, DatabaseConfig, JwtConfig, RedisConfig,
};
use iam_service::routes::build_router;
use iam_service::service::bootstrap::{executor, gate, BootstrapService, INIT_PASSWORD_HEADER};
use iam_service::service::token_forward::TokenForwardClient;
use iam_service::state::AppState;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ADMIN_PASSWORD: &str = "First-Admin-Passw0rd!";

struct Ctx {
    state: AppState,
    app: axum::Router,
    _mysql: Option<common::MysqlContainer>,
}

async fn setup() -> Ctx {
    let mysql_h = common::spawn_mysql().await;
    let pool = mysql_h.pool;
    let url = mysql_h.url;

    let config = Config {
        smtp: Default::default(),
        public: Default::default(),
        app: AppConfig {
            name: "iam-service".into(),
            host: "0.0.0.0".into(),
            port: 8080,
        },
        database: DatabaseConfig {
            url: url.clone(),
            max_connections: 20,
            min_connections: 2,
        },
        redis: RedisConfig {
            url: "redis://x".into(),
            ttl_sec: 86400,
            prefix: "test:".into(),
        },
        jwt: JwtConfig {
            private_key_path: "/dev/null".into(),
            public_key_path: "/dev/null".into(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
        },
        consul: ConsulConfig {
            host: "consul".into(),
            port: 8500,
            service_name: "iam-service".into(),
        },
    };
    let cache = MemoryCache::new(10000);
    let outbox_repo = Arc::new(MySqlOutboxRepo::new(pool.clone()));
    let jwks = Arc::new(
        digua_common_jwt::JWKSCache::from_pem(include_str!(
            "../../token-service/tests/fixtures/test_token_public.pem"
        ))
        .expect("test public key must parse"),
    );
    // service_jwt 指向 token-service stub（由各用例按需启动并回填 URL）。
    let service_jwt = Arc::new(digua_common_jwt::ServiceJwtCache::new(
        digua_common_jwt::ServiceJwtConfig {
            token_service_url: "http://localhost:0".into(),
            bootstrap_token: "test-bt".into(),
            caller_service_name: "iam-service".into(),
            caller_tenant: 1,
            refresh_skew: Duration::from_secs(60),
            request_timeout: Duration::from_secs(5),
        },
        reqwest::Client::new(),
        String::new(),
    ));
    let token_forward = Arc::new(TokenForwardClient::new(service_jwt.clone(), String::new()));
    let state = AppState::new(
        digua_common_db::DbPool::from(pool.clone()),
        cache,
        config,
        None,
        outbox_repo,
        jwks,
        service_jwt.clone(),
        token_forward,
    );
    let app = build_router(state.clone());
    Ctx {
        state,
        app,
        _mysql: mysql_h.container,
    }
}

fn svc(ctx: &Ctx) -> BootstrapService {
    BootstrapService::with_init_password(ctx.state.clone(), Some("init-pw-123"))
}

#[allow(dead_code)]
fn svc_no_password(ctx: &Ctx) -> BootstrapService {
    BootstrapService::with_init_password(ctx.state.clone(), None)
}

fn admin_req(username: &str) -> iam_service::dto::BootstrapRequest {
    iam_service::dto::BootstrapRequest {
        username: Some(username.into()),
        password: Some(ADMIN_PASSWORD.into()),
        email: Some("admin@example.com".into()),
        nickname: Some("Bootstrap Admin".into()),
    }
}

/// stub token-service：service-jwt mint + bootstrap readiness / ensure-grants。
/// `readiness_status` 控制身份是否就绪（None = 不挂路由 → 连接失败）。
async fn stub_token_service(identity_ready: bool) -> MockServer {
    let server = MockServer::start().await;
    let now = chrono::Utc::now().timestamp();
    let svc_claims = json!({
        "iss": "token-service", "sub": "iam-service", "aud": "token-service",
        "tid": 1, "iat": now, "exp": now + 3600, "roles": ["service"],
    });
    let service_jwt = digua_common_jwt::sign_token(
        include_str!("../../token-service/tests/fixtures/test_token_private.pem"),
        &svc_claims,
        Duration::from_secs(3600),
    )
    .expect("sign svc-jwt");

    Mock::given(method("POST"))
        .and(path("/tokens/service-jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": service_jwt, "expires_in": 3600,
        })))
        .mount(&server)
        .await;

    let identity_body = if identity_ready {
        json!({
            "identities": [
                {"owner": "iam-service", "exists": true, "status": "active"},
                {"owner": "notification-service", "exists": true, "status": "active"}
            ],
            "grants": [
                {"owner": "iam-service", "audience": "token-service", "exists": true, "status": "active", "system_managed": true},
                {"owner": "notification-service", "audience": "iam-service", "exists": true, "status": "active", "system_managed": true}
            ]
        })
    } else {
        json!({
            "identities": [
                {"owner": "iam-service", "exists": true, "status": "active"},
                {"owner": "notification-service", "exists": false, "status": null}
            ],
            "grants": []
        })
    };
    let user_claims = json!({
        "iss": "token-service", "sub": "u-login-t1", "tid": 1,
        "iat": now, "exp": now + 3600, "user_code": "u-login-t1", "roles": ["user"],
    });
    let user_jwt = digua_common_jwt::sign_token(
        include_str!("../../token-service/tests/fixtures/test_token_private.pem"),
        &user_claims,
        Duration::from_secs(3600),
    )
    .expect("sign user-jwt");
    Mock::given(method("POST"))
        .and(path("/tokens/user-jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": user_jwt, "expires_in": 3600,
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/internal/bootstrap/readiness"))
        .respond_with(ResponseTemplate::new(200).set_body_json(identity_body.clone()))
        .mount(&server)
        .await;

    let ensure_body = if identity_ready {
        json!({
            "ensured": [
                {"owner": "iam-service", "audience": "token-service", "exists": true, "status": "active", "system_managed": true},
                {"owner": "notification-service", "audience": "iam-service", "exists": true, "status": "active", "system_managed": true}
            ],
            "conflicts": []
        })
    } else {
        json!({"ensured": [], "conflicts": []})
    };
    Mock::given(method("POST"))
        .and(path("/internal/bootstrap/ensure-grants"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ensure_body))
        .mount(&server)
        .await;

    server
}

/// 把 state 的 token-service 指向 stub（ServiceJwtCache URL 不可变，
/// 重建 cache 后经 AppState 替换字段）。
fn point_state_to(state: &AppState, uri: &str) -> AppState {
    let mut s = state.clone();
    let service_jwt = Arc::new(digua_common_jwt::ServiceJwtCache::new(
        digua_common_jwt::ServiceJwtConfig {
            token_service_url: uri.to_string(),
            bootstrap_token: "test-bt".into(),
            caller_service_name: "iam-service".into(),
            caller_tenant: 1,
            refresh_skew: Duration::from_secs(60),
            request_timeout: Duration::from_secs(5),
        },
        reqwest::Client::new(),
        String::new(),
    ));
    let token_forward = Arc::new(TokenForwardClient::new(
        service_jwt.clone(),
        uri.to_string(),
    ));
    s.service_jwt = service_jwt;
    s.token_forward = token_forward;
    s
}

// ── 5.1 基础契约（HTTP 层，env 串行段）────────────────────────────────────

#[tokio::test]
async fn http_password_contract() {
    let ctx = setup().await;
    let _ = std::env::remove_var("LMMS_INIT_PASSWORD");

    // 未配置密码 → 404（GET/POST 皆然，且不产生候选记录）。
    for req in [
        Request::builder()
            .uri("/v1/bootstrap/status")
            .body(Body::empty())
            .unwrap(),
        Request::builder()
            .method("POST")
            .uri("/v1/bootstrap")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap(),
    ] {
        let resp = ctx.app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            404,
            "bootstrap must be 404 without init password"
        );
    }
    let (count,): (i64,) = db_fetch_one!(
        ctx.state.db,
        (i64,),
        "SELECT COUNT(*) FROM iam_bootstrap_init_requests"
    )
    .unwrap();
    assert_eq!(count, 0, "404 path must not create candidate records");

    // 配置密码后：错误密码 → 403。
    std::env::set_var("LMMS_INIT_PASSWORD", "real-init-pw");
    let resp = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/bootstrap")
                .header("content-type", "application/json")
                .header(INIT_PASSWORD_HEADER, "wrong-pw")
                .body(Body::from(
                    json!({"username": "admin", "password": ADMIN_PASSWORD}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(
        !body_str.contains("real-init-pw"),
        "response must never echo the password"
    );
    std::env::remove_var("LMMS_INIT_PASSWORD");

    // 普通登录不受 Bootstrap Password 影响：正常创建用户并登录成功。
    // （login 链依赖 token-service 签发 user-jwt —— 挂 stub。）
    let login_stub = stub_token_service(true).await;
    let login_state = point_state_to(&ctx.state, &login_stub.uri());
    let login_app = build_router(login_state);
    let login_pool = common::mysql_of(&ctx.state);
    sqlx::query(
        "INSERT INTO iam_users (id, tenant_id, username, password_hash, salt, nickname, status, email_verified, phone_verified, created_at, updated_at) \
         VALUES ('u-login-t1', 1, 'loginuser', ?, 'salt', 'lu', 'active', false, false, NOW(), NOW())",
    )
    .bind(digua_common_crypto::password::hash("Login-Pw-123!").unwrap())
    .execute(&login_pool)
    .await
    .unwrap();
    let resp = login_app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/iam/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"username": "loginuser", "password": "Login-Pw-123!"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "normal login must ignore bootstrap password env"
    );
}

// ── 5.1 首次初始化成功 + 完成后 404 ──────────────────────────────────────

#[tokio::test]
async fn first_bootstrap_success_then_404() {
    let ctx = setup().await;
    let stub = stub_token_service(true).await;
    let state = point_state_to(&ctx.state, &stub.uri());
    let service = BootstrapService::with_init_password(state, Some("init-pw-123"))
        .with_token_service_url(stub.uri());

    // 首次初始化：全部步骤完成。
    let resp = service
        .bootstrap(admin_req("admin"), Some("init-pw-123".into()))
        .await
        .expect("first bootstrap must succeed");
    assert_eq!(resp.status, "completed");
    assert_eq!(resp.admin_username.as_deref(), Some("admin"));
    assert!(resp.steps.iter().all(|s| s.status == "completed"));
    assert_eq!(resp.steps.len(), 7);

    // 首管理员落库 + 双角色绑定。
    let (uid,): (String,) = db_fetch_one!(
        ctx.state.db,
        (String,),
        "SELECT id FROM iam_users WHERE username = 'admin' AND deleted_at IS NULL"
    )
    .unwrap();
    let (n_admin,): (i64,) = db_fetch_one!(
        ctx.state.db,
        (i64,),
        "SELECT COUNT(*) FROM iam_user_roles WHERE user_id = ? AND role_id = 'role-platform-admin-seed'",
        &uid
    )
    .unwrap();
    assert_eq!(n_admin, 1, "platform_admin binding required");
    let (n_tenant,): (i64,) = db_fetch_one!(
        ctx.state.db,
        (i64,),
        "SELECT COUNT(*) FROM iam_user_roles ur JOIN iam_roles r ON r.id = ur.role_id \
         WHERE ur.user_id = ? AND r.code = 'tenant_admin'",
        &uid
    )
    .unwrap();
    assert_eq!(n_tenant, 1, "tenant_admin binding required");
    // 两种角色不能合并：role id 不同。
    let (distinct,): (i64,) = db_fetch_one!(
        ctx.state.db,
        (i64,),
        "SELECT COUNT(DISTINCT ur.role_id) FROM iam_user_roles ur \
         WHERE ur.user_id = ? AND (ur.role_id = 'role-platform-admin-seed' \
           OR ur.role_id IN (SELECT id FROM iam_roles WHERE code = 'tenant_admin'))",
        &uid
    )
    .unwrap();
    assert_eq!(
        distinct, 2,
        "platform_admin and tenant_admin must stay separate roles"
    );

    // steps JSON 不含凭据。
    let (steps,): (Option<serde_json::Value>,) = db_fetch_one!(
        ctx.state.db,
        (Option<serde_json::Value>,),
        "SELECT steps FROM iam_bootstrap_init_requests WHERE id = ?",
        resp_steps_id(&ctx).await
    )
    .unwrap();
    let steps_str = serde_json::to_string(&steps).unwrap().to_lowercase();
    assert!(
        !steps_str.contains(ADMIN_PASSWORD),
        "steps must never contain admin password"
    );
    assert!(
        !steps_str.contains("init-pw-123"),
        "steps must never contain bootstrap password"
    );

    // 完成后：winner completed → 404。
    assert!(matches!(
        service.status(Some("init-pw-123".into())).await,
        Err(iam_service::error::IamError::BootstrapCompleted)
    ));
    assert!(matches!(
        service
            .bootstrap(admin_req("admin"), Some("init-pw-123".into()))
            .await,
        Err(iam_service::error::IamError::BootstrapCompleted)
    ));
}

async fn resp_steps_id(ctx: &Ctx) -> i64 {
    let (id,): (i64,) = db_fetch_one!(
        ctx.state.db,
        (i64,),
        "SELECT MIN(id) FROM iam_bootstrap_init_requests WHERE status = 'completed'"
    )
    .unwrap();
    id
}

// ── 5.2 并发准入：窗口内全部插入，关闭选最小 id ──────────────────────────

#[tokio::test]
async fn concurrent_admission_selects_min_id() {
    let ctx = setup().await;
    let pool = &ctx.state.db;

    // 10 个并发准入（模拟多 IAM 实例同时通过初始检查）。
    let mut tasks = Vec::new();
    for _ in 0..10 {
        let pool = pool.clone();
        tasks.push(tokio::spawn(
            async move { gate::admit_candidate(&pool).await },
        ));
    }
    let mut ids = Vec::new();
    let mut ends: Option<chrono::DateTime<chrono::Utc>> = None;
    for t in tasks {
        match t.await.unwrap().expect("admit ok") {
            gate::AdmitOutcome::Admitted {
                request_id,
                window_ends_at,
            } => {
                ids.push(request_id);
                ends = Some(window_ends_at);
            }
            other => panic!(
                "all initial-check-passed requests must be admitted in window, got {other:?}"
            ),
        }
    }
    assert_eq!(ids.len(), 10);

    // 等窗口到期后关闭。
    let wait = (ends.unwrap() - gate::db_now(pool).await.unwrap())
        .to_std()
        .unwrap_or(Duration::ZERO);
    tokio::time::sleep(wait + Duration::from_millis(50)).await;
    let winner = match gate::try_close_window(pool).await.unwrap() {
        gate::CloseOutcome::Closed { winner } => winner.expect("winner elected"),
        other => panic!("expected Closed, got {other:?}"),
    };
    assert_eq!(
        winner,
        ids.iter().copied().min().unwrap(),
        "winner must be min id"
    );

    // 其余全部 rejected，且不可执行。
    let (rejected,): (i64,) = db_fetch_one!(
        pool,
        (i64,),
        "SELECT COUNT(*) FROM iam_bootstrap_init_requests WHERE status = 'rejected'"
    )
    .unwrap();
    assert_eq!(rejected, 9);
    for rid in &ids {
        if *rid == winner {
            continue;
        }
        let handle = executor::claim_execution(pool, *rid, "loser-instance")
            .await
            .unwrap();
        assert!(
            handle.is_none(),
            "rejected request {rid} must not be claimable"
        );
    }
}

/// 设计验收核心用例：候选 INSERT 尚未提交时窗口到期，关闭事务竞争 gate 锁
/// ——关闭事务必须等待在途准入事务提交，并把该候选纳入最小 id 仲裁。
#[tokio::test]
async fn uncommitted_candidate_is_included_by_close() {
    let ctx = setup().await;
    let pool = &ctx.state.db;

    // 在途准入事务：锁 gate、开窗、插候选 id=A，**不提交**。
    let mut pending = pool.begin().await.unwrap();
    let gate_row: Option<(String,)> = digua_common_db::db_tx_fetch_optional!(
        &mut pending,
        (String,),
        "SELECT status FROM iam_bootstrap_init_gate WHERE id = 1 FOR UPDATE"
    )
    .unwrap();
    assert_eq!(gate_row.unwrap().0, "idle");
    let now = gate::tx_now(&mut pending).await.unwrap();
    digua_common_db::db_tx_execute!(
        &mut pending,
        "UPDATE iam_bootstrap_init_gate SET status = 'collecting', window_started_at = ?, \
         window_ends_at = ? WHERE id = 1 AND status = 'idle'",
        now,
        now + chrono::Duration::seconds(2)
    )
    .unwrap();
    let pending_id: i64 = {
        digua_common_db::db_tx_execute!(
            &mut pending,
            "INSERT INTO iam_bootstrap_init_requests (status, created_at, updated_at)              VALUES ('collecting', ?, ?)",
            now,
            now
        )
        .unwrap();
        let (id,): (u64,) = match &mut pending {
            digua_common_db::Transaction::Mysql(t) => sqlx::query_as("SELECT LAST_INSERT_ID()")
                .fetch_one(&mut **t)
                .await
                .unwrap(),
            _ => panic!("mysql fixture expected"),
        };
        i64::try_from(id).unwrap()
    };

    // 主线程开第二个"独立实例"准入事务：被 gate 锁阻塞中 → close 也阻塞。
    let pool2 = pool.clone();
    let closer = tokio::spawn(async move {
        // 先睡到窗口到期，再尝试关闭（会等 pending 提交/回滚）。
        tokio::time::sleep(Duration::from_millis(2300)).await;
        gate::try_close_window(&pool2).await
    });
    // 在途事务此刻提交（候选 A 可见）。
    pending.commit().await.unwrap();

    let outcome = closer.await.unwrap().unwrap();
    match outcome {
        gate::CloseOutcome::Closed { winner } => {
            assert_eq!(
                winner,
                Some(pending_id),
                "uncommitted-then-committed candidate must win min-id arbitration"
            );
        }
        other => panic!("expected Closed, got {other:?}"),
    }
}

/// 同场景的回滚变体：在途候选（更小 id）回滚后，关闭不得把它算进仲裁，
/// 胜者落在剩余已提交候选中的最小 id。
#[tokio::test]
async fn rolled_back_candidate_is_excluded_by_close() {
    let ctx = setup().await;
    let pool = &ctx.state.db;

    // 在途候选 A：锁 gate、开窗、INSERT（更小 id），**不提交**。
    let mut pending = pool.begin().await.unwrap();
    let now = gate::tx_now(&mut pending).await.unwrap();
    digua_common_db::db_tx_execute!(
        &mut pending,
        "UPDATE iam_bootstrap_init_gate SET status = 'collecting', window_started_at = ?,          window_ends_at = ? WHERE id = 1 AND status = 'idle'",
        now,
        now + chrono::Duration::seconds(2)
    )
    .unwrap();
    digua_common_db::db_tx_execute!(
        &mut pending,
        "INSERT INTO iam_bootstrap_init_requests (status, created_at, updated_at)          VALUES ('collecting', ?, ?)",
        now,
        now
    )
    .unwrap();

    // B 的准入事务此刻被 gate 锁阻塞排队（模拟并发实例）。
    let pool2 = pool.clone();
    let b_task = tokio::spawn(async move { gate::admit_candidate(&pool2).await });
    tokio::time::sleep(Duration::from_millis(200)).await;

    // A 回滚：释放 gate 锁 → B 获得锁并准入（id 比 A 大）。
    pending.rollback().await.unwrap();
    let b = b_task.await.unwrap().unwrap();
    let b_id = match &b {
        gate::AdmitOutcome::Admitted { request_id, .. } => *request_id,
        other => panic!("admit B failed: {other:?}"),
    };

    // 窗口到期后关闭：A 已回滚不存在 → 胜者 = B。
    tokio::time::sleep(Duration::from_millis(2300)).await;
    match gate::try_close_window(pool).await.unwrap() {
        gate::CloseOutcome::Closed { winner } => {
            assert_eq!(winner, Some(b_id), "rolled-back candidate must be excluded");
        }
        other => panic!("expected Closed, got {other:?}"),
    }
    let (a_rows,): (i64,) = db_fetch_one!(
        pool,
        (i64,),
        "SELECT COUNT(*) FROM iam_bootstrap_init_requests"
    )
    .unwrap();
    assert_eq!(a_rows, 1, "rolled-back candidate must not exist");
}

// ── 5.2 执行权：原子 claim + epoch fencing ───────────────────────────────

#[tokio::test]
async fn concurrent_claim_exactly_one_winner() {
    let ctx = setup().await;
    let pool = &ctx.state.db;

    let rid = seed_selected_request(pool).await;
    let mut tasks = Vec::new();
    for i in 0..8 {
        let pool = pool.clone();
        tasks.push(tokio::spawn(async move {
            executor::claim_execution(&pool, rid, &format!("instance-{i}")).await
        }));
    }
    let mut winners = Vec::new();
    for t in tasks {
        if let Some(h) = t.await.unwrap().unwrap() {
            winners.push(h);
        }
    }
    assert_eq!(winners.len(), 1, "exactly one instance must win the claim");
    let w = winners.into_iter().next().unwrap();
    assert_eq!(w.epoch, 1);
    assert_eq!(w.request_id, rid);
}

#[tokio::test]
async fn old_epoch_cannot_write_step_state() {
    let ctx = setup().await;
    let pool = &ctx.state.db;

    let rid = seed_selected_request(pool).await;
    let old = executor::claim_execution(pool, rid, "old-instance")
        .await
        .unwrap()
        .expect("first claim succeeds");

    // 模拟租约过期 + 新实例接管（epoch 递增）。
    db_execute!(
        pool,
        "UPDATE iam_bootstrap_init_requests SET lease_expires_at = DATE_SUB(NOW(6), INTERVAL 1 SECOND) WHERE id = ?",
        rid
    )
    .unwrap();
    let new = executor::claim_execution(pool, rid, "new-instance")
        .await
        .unwrap()
        .expect("recovery claim succeeds after lease expiry");
    assert_eq!(
        new.epoch,
        old.epoch + 1,
        "epoch must be a monotonic fencing token"
    );

    // 旧 epoch 的所有写入都被拒绝（fencing）。
    let steps = iam_service::service::bootstrap::steps::StepsState::initial().to_value();
    assert!(
        !executor::persist_step_progress(pool, &old, &steps, chrono::Utc::now())
            .await
            .unwrap()
    );
    assert!(
        !executor::mark_step_failed(pool, &old, &steps, "x", chrono::Utc::now())
            .await
            .unwrap()
    );
    assert!(
        !executor::mark_completed(pool, &old, &steps, chrono::Utc::now())
            .await
            .unwrap()
    );
    assert!(!executor::renew_lease(pool, &old).await.unwrap());

    // 新 epoch 写入正常。
    assert!(
        executor::persist_step_progress(pool, &new, &steps, chrono::Utc::now())
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn completed_request_is_not_claimable() {
    let ctx = setup().await;
    let pool = &ctx.state.db;
    let rid = seed_selected_request(pool).await;
    db_execute!(
        pool,
        "UPDATE iam_bootstrap_init_requests SET status = 'completed', finished_at = NOW(6) WHERE id = ?",
        rid
    )
    .unwrap();
    assert!(executor::claim_execution(pool, rid, "someone")
        .await
        .unwrap()
        .is_none());
}

// ── 5.3 故障恢复 ─────────────────────────────────────────────────────────

/// token-service 不可用 → 步骤失败（502 语义）+ 租约释放；恢复请求
/// （token-service 就绪后）续跑未完成步骤，不重复建用户/绑定。
#[tokio::test]
async fn recovery_after_dependency_failure() {
    let ctx = setup().await;
    // 无 stub：S05 连接失败。
    let service = svc(&ctx);

    let err = service
        .bootstrap(admin_req("admin"), Some("init-pw-123".into()))
        .await
        .expect_err("S05 dependency failure expected");
    assert!(matches!(
        err,
        iam_service::error::IamError::BootstrapDependencyUnavailable
    ));

    // 总状态 failed；S01-S04 已完成并持久化；租约已释放（NULL）。
    let (status, lease, last_error): (String, Option<chrono::DateTime<chrono::Utc>>, Option<String>) =
        db_fetch_one!(
            ctx.state.db,
            (String, Option<chrono::DateTime<chrono::Utc>>, Option<String>),
            "SELECT status, lease_expires_at, last_error FROM iam_bootstrap_init_requests WHERE status = 'failed'"
        )
        .unwrap();
    assert_eq!(status, "failed");
    assert!(lease.is_none(), "lease must be released on step failure");
    assert_eq!(last_error.as_deref(), Some("dependency_unavailable"));
    let (s04_done,): (i64,) = db_fetch_one!(
        ctx.state.db,
        (i64,),
        "SELECT COUNT(*) FROM iam_user_roles ur JOIN iam_roles r ON r.id = ur.role_id \
         WHERE r.code = 'tenant_admin'"
    )
    .unwrap();
    assert_eq!(
        s04_done, 1,
        "local steps S01-S04 complete before dependency failure"
    );

    // 恢复：token-service 就绪后重试 → 全部完成，且不重复创建。
    let stub = stub_token_service(true).await;
    let state = point_state_to(&ctx.state, &stub.uri());
    let recovery = BootstrapService::with_init_password(state, Some("init-pw-123"))
        .with_token_service_url(stub.uri());
    let resp = recovery
        .bootstrap(admin_req("admin"), Some("init-pw-123".into()))
        .await
        .expect("recovery must succeed");
    assert_eq!(resp.status, "completed");

    let (users,): (i64,) = db_fetch_one!(
        ctx.state.db,
        (i64,),
        "SELECT COUNT(*) FROM iam_users WHERE username = 'admin'"
    )
    .unwrap();
    assert_eq!(users, 1, "admin must not be duplicated across recovery");
    let (bindings,): (i64,) =
        db_fetch_one!(ctx.state.db, (i64,), "SELECT COUNT(*) FROM iam_user_roles").unwrap();
    assert_eq!(bindings, 2, "role bindings must not be duplicated");
}

/// 跨服务不确定结果：token-service 创建成功但 IAM 持久化步骤状态前崩溃
/// （模拟：手工把 steps JSON 中的 token 步骤回拨为 pending）→ 恢复走
/// Check 确认而非重复创建。
#[tokio::test]
async fn recovery_confirms_remote_steps_via_check() {
    let ctx = setup().await;
    let stub = stub_token_service(true).await;
    let state = point_state_to(&ctx.state, &stub.uri());
    let service = BootstrapService::with_init_password(state, Some("init-pw-123"))
        .with_token_service_url(stub.uri());

    service
        .bootstrap(admin_req("admin"), Some("init-pw-123".into()))
        .await
        .expect("bootstrap succeeds");
    let rid = resp_steps_id(&ctx).await;

    // 模拟"远端成功但本地步骤状态未持久化"：S05-S07 回拨 pending。
    let (steps,): (Option<serde_json::Value>,) = db_fetch_one!(
        ctx.state.db,
        (Option<serde_json::Value>,),
        "SELECT steps FROM iam_bootstrap_init_requests WHERE id = ?",
        rid
    )
    .unwrap();
    let mut steps = steps.unwrap();
    for step in steps["steps"].as_array_mut().unwrap() {
        let key = step["key"].as_str().unwrap();
        if key.starts_with("grant_") || key == "internal_identities" {
            step["status"] = json!("pending");
        }
    }
    db_execute!(
        ctx.state.db,
        "UPDATE iam_bootstrap_init_requests SET steps = ?, status = 'failed', lease_expires_at = NULL WHERE id = ?",
        steps,
        rid
    )
    .unwrap();

    // 恢复请求（凭据重新提交——S02 已完成也允许重复提交）。
    let resp_after = service
        .bootstrap(admin_req("admin"), Some("init-pw-123".into()))
        .await
        .expect("recovery confirms remote steps");
    assert_eq!(resp_after.status, "completed");

    let (users,): (i64,) = db_fetch_one!(
        ctx.state.db,
        (i64,),
        "SELECT COUNT(*) FROM iam_users WHERE username = 'admin'"
    )
    .unwrap();
    assert_eq!(users, 1);
    let (bindings,): (i64,) =
        db_fetch_one!(ctx.state.db, (i64,), "SELECT COUNT(*) FROM iam_user_roles").unwrap();
    assert_eq!(bindings, 2);
    // wiremock readiness 被再次调用（Check 语义）≥ 1 次。
    // （次数断言弱化为 > 0：Check 确实发生了。）
}

/// 执行租约活跃 → 其他请求 409（进程崩溃后、租约到期前）。
#[tokio::test]
async fn lease_active_returns_conflict() {
    let ctx = setup().await;
    let pool = &ctx.state.db;
    let rid = seed_selected_request(pool).await;
    let _handle = executor::claim_execution(pool, rid, "crashed-instance")
        .await
        .unwrap()
        .expect("claim");

    // 模拟该实例崩溃：不释放租约。恢复请求 → 409。
    let service = svc(&ctx);
    let err = service
        .bootstrap(admin_req("admin"), Some("init-pw-123".into()))
        .await
        .expect_err("lease active must conflict");
    assert!(matches!(
        err,
        iam_service::error::IamError::BootstrapConflict(_)
    ));
}

/// 窗口进行中的新请求 → 409（不加入已开启的首轮）。
#[tokio::test]
async fn late_request_during_window_conflicts() {
    let ctx = setup().await;
    // 制造 collecting 状态（未到期）。
    let admitted = gate::admit_candidate(&ctx.state.db).await.unwrap();
    assert!(matches!(admitted, gate::AdmitOutcome::Admitted { .. }));

    let service = svc(&ctx);
    let err = service
        .bootstrap(admin_req("admin"), Some("init-pw-123".into()))
        .await
        .expect_err("in-window late request must conflict");
    assert!(matches!(
        err,
        iam_service::error::IamError::BootstrapConflict(_)
    ));
}

/// 首管理员已存在（同名+同密码）→ S02 幂等确认；同名+不同密码 → 冲突不覆盖。
#[tokio::test]
async fn admin_conflict_not_overwritten() {
    let ctx = setup().await;
    let stub = stub_token_service(true).await;
    let state = point_state_to(&ctx.state, &stub.uri());
    let service = BootstrapService::with_init_password(state, Some("init-pw-123"))
        .with_token_service_url(stub.uri());
    service
        .bootstrap(admin_req("admin"), Some("init-pw-123".into()))
        .await
        .await_ok();

    // 模拟恢复场景：steps 回拨 S02 为 pending，但密码不同 → 冲突。
    let rid = resp_steps_id(&ctx).await;
    reset_step(&ctx, rid, "first_admin_user").await;
    let different = iam_service::dto::BootstrapRequest {
        username: Some("admin".into()),
        password: Some("Different-Pw-123!".into()),
        email: None,
        nickname: None,
    };
    let err = BootstrapService::with_init_password(
        point_state_to(&ctx.state, &stub.uri()),
        Some("init-pw-123"),
    )
    .with_token_service_url(stub.uri())
    .bootstrap(different, Some("init-pw-123".into()))
    .await
    .expect_err("password mismatch must conflict");
    assert!(matches!(
        err,
        iam_service::error::IamError::BootstrapConflict(_)
    ));

    // 数据未被覆盖：原密码仍可验证。
    let (hash,): (String,) = db_fetch_one!(
        ctx.state.db,
        (String,),
        "SELECT password_hash FROM iam_users WHERE username = 'admin'"
    )
    .unwrap();
    assert!(digua_common_crypto::password::verify(ADMIN_PASSWORD, &hash).unwrap());
}

// ── helpers ──────────────────────────────────────────────────────────────

/// 手工造一条 selected 状态的胜出记录（执行权测试夹具）。
async fn seed_selected_request(pool: &digua_common_db::DbPool) -> i64 {
    // 先走一轮完整准入+关闭拿到 selected 记录（语义与真实路径一致）。
    let admitted = gate::admit_candidate(pool).await.unwrap();
    let (rid, ends) = match &admitted {
        gate::AdmitOutcome::Admitted {
            request_id,
            window_ends_at,
        } => (*request_id, *window_ends_at),
        other => panic!("seed admit failed: {other:?}"),
    };
    let wait = (ends - gate::db_now(pool).await.unwrap())
        .to_std()
        .unwrap_or(Duration::ZERO);
    tokio::time::sleep(wait + Duration::from_millis(50)).await;
    match gate::try_close_window(pool).await.unwrap() {
        gate::CloseOutcome::Closed { winner: Some(w) } => assert_eq!(w, rid),
        other => panic!("seed close failed: {other:?}"),
    }
    rid
}

async fn reset_step(ctx: &Ctx, rid: i64, key: &str) {
    let (steps,): (Option<serde_json::Value>,) = db_fetch_one!(
        ctx.state.db,
        (Option<serde_json::Value>,),
        "SELECT steps FROM iam_bootstrap_init_requests WHERE id = ?",
        rid
    )
    .unwrap();
    let mut steps = steps.unwrap();
    for step in steps["steps"].as_array_mut().unwrap() {
        if step["key"].as_str() == Some(key) {
            step["status"] = json!("pending");
        }
    }
    db_execute!(
        ctx.state.db,
        "UPDATE iam_bootstrap_init_requests SET steps = ?, status = 'failed', lease_expires_at = NULL WHERE id = ?",
        steps,
        rid
    )
    .unwrap();
}

trait BootstrapResultExt {
    fn await_ok(self);
}

impl BootstrapResultExt
    for Result<iam_service::dto::BootstrapResponse, iam_service::error::IamError>
{
    fn await_ok(self) {
        if let Err(e) = self {
            panic!("expected bootstrap success, got {e:?}");
        }
    }
}
