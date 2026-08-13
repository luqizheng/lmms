---
name: coder-rust-unit-test
description: |
  Scaffold unit, integration, and smoke tests for existing Rust microservices following this
  repo's two-tier test model (host for pure unit, Docker for testcontainers).
  Triggers: "add tests", "write unit test", "TDD", "测试", "覆盖率", "mock",
  "mockall", "sqlx mock", "#[cfg(test)]", "test stub", "tower::ServiceExt::oneshot",
  "wiremock"
---

# coder-rust-unit-test Skill

为 Rust 微服务编写单元/集成/冒烟测试。

## 何时用

用户说"加测试"、"写单元测试"、"TDD"、"测试"、"覆盖率"、"mock"、"mockall"、
"sqlx mock"、"#[cfg(test)]"、"test stub"、或想为一个路由/service 补充测试桩时调用。

## 前置

- 纯单元测试（本机跑）：`cc`/`gcc` + `rustup target add x86_64-unknown-linux-gnu`
- testcontainers 集成测试（MySQL/Redis）：Docker daemon 运行中
- 建议先跑 `pnpm run assets:download` 预下载 swagger-ui zip，避免 build.rs 触发网络

## 本仓测试三层架构

| 层级 | 目录约定 | 运行命令 | 依赖要求 | 典型耗时 |
|------|----------|----------|----------|----------|
| **unit**（纯单元） | `src/**/*.rs` 内 `#[cfg(test)] mod tests` | `CARGO_TARGET_DIR=/tmp/<svc>-target SWAGGER_UI_DOWNLOAD_URL=file://$(pwd)/infrastructure/vendor/swagger-ui-v5.17.14.zip cargo test -p <svc> --lib` | 无外部依赖 | < 5s |
| **integration**（集成） | `services/<svc>/tests/*.rs` | `node scripts/run-tests-docker.js <svc>` | Docker daemon（testcontainers 起 MySQL/Redis） | 30-90s |
| **e2e**（端到端） | `tests/e2e/` | `pnpm run test:e2e` | 全部基础设施（MySQL/Redis/Consul） + `pnpm install` | 2-10min |

### 两条执行路径

**纯单元测试（不依赖 MySQL / 不走 Docker）：**
```bash
# 在宿主机跑 src/ 下的 #[cfg(test)] / #[tokio::test]
# target/ 若被 docker volume 抢成 root-owned，用独立目录绕开
CARGO_TARGET_DIR=/tmp/<svc>-target \
  SWAGGER_UI_DOWNLOAD_URL=file://$(pwd)/infrastructure/vendor/swagger-ui-v5.17.14.zip \
  cargo test -p <svc> --lib
```

**集成测试（依赖 testcontainers MySQL，必须走 Docker）：**
```bash
# 容器内跑 tests/ 目录（testcontainers 需 docker daemon）
node scripts/run-tests-docker.js <svc>
# 或等价底层命令：
docker run --rm --network host -v ${PWD}:/app \
  -v coder-cargo-registry:/usr/local/cargo/registry \
  -v coder-cargo-git:/usr/local/cargo/git \
  -v coder-cargo-target:/app/target \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -w /app \
  zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl \
  cargo test --target x86_64-unknown-linux-musl -p <svc> -- --nocapture
```

## 模板 1：纯函数单元测试（src/ 内）

适合业务逻辑函数、数据转换、模型方法。

文件：`services/<svc>/src/service/my_logic.rs`

```rust
use serde_json::{Map, Value};

pub fn canonicalize(value: &Value) -> Value { /* ... */ }

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn same_input_yields_same_output() {
        let a = json!({"a": 1, "b": 2});
        let b = json!({"b": 2, "a": 1});
        assert_eq!(canonicalize(&a), canonicalize(&b));
    }

    #[test]
    fn different_input_yields_different_output() {
        let a = json!({"a": 1});
        let b = json!({"a": 2});
        assert_ne!(canonicalize(&a), canonicalize(&b));
    }
}
```

运行：`cargo test -p <svc> --lib`

参考：`services/prue_backend/src/service/etag.rs`

## 模板 2：axum 路由层 Smoke Test

使用 `tower::util::ServiceExt::oneshot` 直接测 Router，不起 HTTP 服务器。
本模板已在 `coder-add-route` Step 4 脚手架生成，可补充 body 断言。

文件：`services/<svc>/tests/my_feature_test.rs`

```rust
use crate::error::AppResult;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use tower::util::ServiceExt;

#[tokio::test]
async fn create_item_returns_201() -> AppResult<()> {
    let app = Router::new()
        .route("/items", axum::routing::post(crate::routes::items::create_item))
        .with_state(mock_state());

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/items")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"name":"test"}"#))?,
        )
        .await?;

    assert_eq!(res.status(), StatusCode::CREATED);
    Ok(())
}
```

运行：`CARGO_TARGET_DIR=/tmp/<svc>-target cargo test -p <svc> --test my_feature_test`

注意：此为 smoke test，只验证路由可达 + status code。完整 body 断言在实现完成后补充。

参考：`services/api-gateway/tests/middleware_test.rs`（同步 `#[test]`）
参考：`coder-add-route/SKILL.md` Step 4

## 模板 3：sqlx Repository 层集成测试（testcontainers MySQL）

文件：`services/<svc>/tests/my_repo_test.rs`

```rust
mod common;

use my_service::repository::item_repo;

#[tokio::test]
async fn save_and_find_by_id() {
    let handle = common::spawn_mysql().await;
    let pool = handle.pool;

    item_repo::save(&pool, "item-1", "owner-a")
        .await
        .unwrap();

    let found = item_repo::find_by_id(&pool, "item-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.name, "item-1");
    assert_eq!(found.owner, "owner-a");
}
```

`services/<svc>/tests/common/mod.rs` 的标准结构（直接拷贝后改服务名）：

```rust
//! Integration test helper：起 MySQL 容器 + 跑 migrations
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::mysql::Mysql;
use my_service::db::DbPool;

pub const MIGRATION_SQL: &str = include_str!("../../migrations/mysql/V001__init.sql");

pub async fn spawn_mysql() -> DbPool {
    // 两条路径：TEST_MYSQL_HOST/PORT 环境变量（run-tests-docker.js）
    // 或本地 testcontainers
    if let (Ok(host), Ok(port)) = (
        std::env::var("TEST_MYSQL_HOST"),
        std::env::var("TEST_MYSQL_PORT"),
    ) {
        return spawn_shared(host, port).await;
    }
    let container = Mysql::default()
        .with_cmd(["--innodb-use-native-aio=0"])
        .start()
        .await
        .expect("start mysql");
    let host = container.get_host().await.expect("host");
    let port = container.get_host_port_ipv4(3306).await.expect("port");
    let url = format!("mysql://root@{}:{}/test", host, port);
    let pool = build_pool(&url).await;
    run_migrations(&pool).await;
    std::mem::forget(container); // 保持容器存活
    DbPool::Mysql(pool)
}

async fn spawn_shared(host: String, port: String) -> DbPool {
    let port: u16 = port.parse().unwrap();
    let db_name = format!(
        "test_{}_{}_{}",
        env!("CARGO_PKG_NAME"),
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );
    let admin_url = format!("mysql://root@{host}:{port}/");
    let admin = MySqlPool::connect(&admin_url).await.expect("admin");
    sqlx::query(&format!("CREATE DATABASE `{db_name}` CHARACTER SET utf8mb4"))
        .execute(&admin).await.expect("create db");
    admin.close().await;
    let url = format!("mysql://root@{host}:{port}/{db_name}");
    let pool = build_pool(&url).await;
    run_migrations(&pool).await;
    DbPool::Mysql(pool)
}

async fn build_pool(url: &str) -> MySqlPool {
    MySqlPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(5))
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("SET time_zone = '+00:00'").execute(conn).await?;
                Ok(())
            })
        })
        .connect(url).await.expect("connect")
}

async fn run_migrations(pool: &MySqlPool) {
    let cleaned = MIGRATION_SQL.lines()
        .filter(|l| !l.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n");
    for stmt in cleaned.split(';') {
        let s = stmt.trim();
        if s.is_empty() { continue; }
        sqlx::query(s).execute(pool).await.expect("migration");
    }
}
```

运行（必须走 Docker）：
```bash
node scripts/run-tests-docker.js <svc>
```

参考：`services/token-service/tests/common/mod.rs`（最完整的双路径实现）

## 模板 4：跨服务 HTTP Mock（wiremock）

本仓跨服务调用使用 `wiremock`（非 mockall）做 HTTP mock。
真实场景：`notification-service` 的 `IamMemberClient` 测试。

文件：`services/<svc>/tests/iam_client_test.rs`

```rust
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn by_role_passes_tenant_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/iam/internal/users/by-role/admin"))
        .and(query_param("tenant_id", "2"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(serde_json::json!({
                "items": ["u1", "u2"], "page": 1, "size": 1000, "total": 2, "truncated": false
            })))
        .expect(1)
        .mount(&server)
        .await;

    let client = build_iam_client(server.uri()).await;
    let result = client.by_role("admin", 2).await.unwrap();
    assert_eq!(result, vec!["u1", "u2"]);
}
```

`wiremock` 在本仓 workspace Cargo.toml 中版本为 `0.6`，dev-dependencies 已声明。
mockall（版本 `0.13`）作为 dev-dependency 存在于各服务，但当前测试实践偏好 wiremock
做 HTTP 层 mock，以保持与真实网络行为的一致性。

运行（必须走 Docker，wiremock 本身不需特殊权限）：
```bash
node scripts/run-tests-docker.js <svc>
```

参考：`services/notification-service/tests/iam_client_test.rs`

## 模板 5：JWT/Principal Extractor 单元测试

所有 auth 相关 handler 使用 `libs/jwt-ctx::Principal` 从请求中提取身份。
测试时用 `Principal::from_jwt_payload()` 绕过签名验证（仅 decode，Phase 5-A 路径）。

```rust
use digua_jwt_ctx::Principal;

#[test]
fn principal_decodes_claims() {
    // 用 jwt_simple 生成测试 token
    let key_pair = RS256KeyPair::generate(2048).unwrap();
    let claims = // ... build JWTClaims with sub/tid/roles/permissions ...
    let token = key_pair.sign(claims).unwrap();

    // 从 token payload decode（不做签名验证，用于测试）
    let principal = Principal::from_jwt_payload(&token).unwrap();
    assert_eq!(principal.user_id, "user-123");
    assert_eq!(principal.tenant_id, 42);
    assert!(principal.has_role("admin"));
    assert!(principal.has_permission("items:read"));
}

#[test]
fn principal_rejects_malformed_token() {
    assert!(Principal::from_jwt_payload("not.a.jwt").is_err());
}
```

若需在路由测试中注入 Principal：用 `Authorization: Bearer <token>` header 触发
`Principal extractor`，或直接在 handler 层用已知 Principal 调用业务函数。

参考：`libs/jwt-ctx/src/principal.rs`（含 `#[deprecated] from_jwt_payload` 注释说明仅用于测试）

## TDD 工作流

```
1. RED    → 写一个 failing test，只描述期望行为
2. GREEN  → 用最小代码让测试通过（哪怕 hardcode）
3. REFACTOR → 重构业务逻辑，测试保持绿色
4. 补充边界 case 和 body 断言
```

每个新路由脚手架已含 Step 4 smoke test，补充步骤：
- 在 `services/<svc>/tests/<feature>_test.rs` 补充 body 断言
- 补充错误路径（400 / 401 / 403 / 404）
- 补充 repository 层 testcontainers 测试（模板 3）

## 覆盖率目标

| 层级 | 最低覆盖率目标 | 典型覆盖内容 |
|------|--------------|-------------|
| unit | 70% 行覆盖 | 纯函数、模型序列化、数据转换 |
| integration | 50% 行覆盖 | repository 方法、 service 层业务逻辑 |
| e2e | 关键路径覆盖 | 登录/创建/查询/删除 全链路 |

运行覆盖率：
```bash
CARGO_TARGET_DIR=/tmp/<svc>-target \
  cargo test -p <svc> --lib -- --cov-append \
  && cargo coverage -p <svc>
```

注：本仓暂未强制 CI 覆盖率门禁，以手动检查为主。

## 验证

1. 纯单元测试编译：`CARGO_TARGET_DIR=/tmp/<svc>-target cargo test -p <svc> --lib --no-run`
2. 单元测试通过：`CARGO_TARGET_DIR=/tmp/<svc>-target cargo test -p <svc> --lib`
3. 集成测试（Docker）：`node scripts/run-tests-docker.js <svc>`
4. 检查新增测试文件无 import 错误：`cargo check -p <svc> --tests`
