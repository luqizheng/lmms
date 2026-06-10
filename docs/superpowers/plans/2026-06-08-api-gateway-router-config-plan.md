# API Gateway 多文件路由配置 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 api-gateway 单文件路由配置拆为 `routes/*.router.toml`，引入重写规则链与手工触发的 `validate`/`reload` 命令。

**Architecture:** 启动期从 `routes/` 目录并行解析所有 `*.router.toml` → 合并校验 → 注入 `ArcSwap<RouteTable>`。运行时 `forward_to_service` 命中路由后调用 `rewrite::apply` 改写请求，再转发。`validate` 子命令只校验不装载；`reload` 重新解析并原子切换 ArcSwap。

**Tech Stack:** Rust + axum + tokio + `arc-swap` + `clap`，TOML via `serde` + `toml` crate。

---

## File Structure

**新建：**
- `services/api-gateway/src/routes_loader.rs`：扫描 `routes/` + 解析 + 合并校验
- `services/api-gateway/src/rewrite.rs`：`Rewrite` 枚举 + `apply` 函数
- `services/api-gateway/src/cli.rs`：`validate` / `reload` / `serve` 子命令
- `services/api-gateway/tests/routes_loader_test.rs`
- `services/api-gateway/tests/rewrite_test.rs`
- `services/api-gateway/tests/validate_cli_test.rs`
- `services/api-gateway/tests/reload_test.rs`
- `docker/config/api-gateway/routes/auth.router.toml`
- `docker/config/api-gateway/routes/tokens.router.toml`
- `docker/config/api-gateway/routes/users.router.toml`
- `docker/config/api-gateway/routes/roles.router.toml`
- `docker/config/api-gateway/routes/permissions.router.toml`

**修改：**
- `services/api-gateway/Cargo.toml`：新增 `arc-swap`、`clap`、`toml`、`glob`
- `services/api-gateway/src/config.rs`：`RouteConfig` 拆为精确 match + 通配 + rewrite
- `services/api-gateway/src/lib.rs`：注册 `routes_loader` / `rewrite` / `cli`，`AppState` 增加 `ArcSwap<RouteTable>`
- `services/api-gateway/src/main.rs`：解析 CLI 子命令，`serve` 时启动 router + 注册 SIGHUP
- `services/api-gateway/src/router.rs`：命中路由后调用 `rewrite::apply`
- `docker/config/api-gateway/api-gateway.toml`：删除 `[[routes]]` 块

---

## Task 1：更新 Cargo.toml 依赖

**Files:**
- Modify: `services/api-gateway/Cargo.toml`

- [ ] **Step 1：添加依赖**

在 `[dependencies]` 末尾追加：

```toml
arc-swap = "1"
clap = { version = "4", features = ["derive"] }
toml = "0.8"
glob = "0.3"
```

- [ ] **Step 2：构建确认**

Run: `cargo build -p api-gateway`
Expected: 成功，无 warning。

- [ ] **Step 3：提交**

```bash
git add services/api-gateway/Cargo.toml Cargo.lock
git commit -m "build(api-gateway): add arc-swap, clap, toml, glob"
```

---

## Task 2：创建 routes/ 配置目录与初始文件

**Files:**
- Create: `docker/config/api-gateway/routes/auth.router.toml`
- Create: `docker/config/api-gateway/routes/tokens.router.toml`
- Create: `docker/config/api-gateway/routes/users.router.toml`
- Create: `docker/config/api-gateway/routes/roles.router.toml`
- Create: `docker/config/api-gateway/routes/permissions.router.toml`
- Modify: `docker/config/api-gateway/api-gateway.toml`

- [ ] **Step 1：写 auth.router.toml**

`docker/config/api-gateway/routes/auth.router.toml`：

```toml
[[route]]
match = "/auth/**"
service = "auth-service"
public = true
```

- [ ] **Step 2：写 tokens.router.toml**

```toml
[[route]]
match = "/tokens/**"
service = "token-server"
public = true
```

- [ ] **Step 3：写 users.router.toml**

```toml
[[route]]
match = "/users/**"
service = "user-service"
public = false
```

- [ ] **Step 4：写 roles.router.toml**

```toml
[[route]]
match = "/roles/**"
service = "user-service"
public = false
```

- [ ] **Step 5：写 permissions.router.toml**

```toml
[[route]]
match = "/permissions/**"
service = "user-service"
public = false
```

- [ ] **Step 6：删除 api-gateway.toml 的 routes 块**

修改 `docker/config/api-gateway/api-gateway.toml`，删除第 13-39 行（注释与 5 条 `[[routes]]`）。文件保留 `server`/`redis`/`consul_addr`/`jwks_public_key_path`/`issuer`。

- [ ] **Step 7：提交**

```bash
git add docker/config/api-gateway/
git commit -m "feat(config): split api-gateway routes into routes/*.router.toml"
```

---

## Task 3：改写 RouteConfig 与 RouteTable（config.rs）

**Files:**
- Modify: `services/api-gateway/src/config.rs`

- [ ] **Step 1：写失败的 compile 测试**

`services/api-gateway/src/config.rs` 顶部新增：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn route_config_parses_match_and_service() {
        let toml = r#"
            [[route]]
            match = "/users/**"
            service = "user-service"
            public = false
        "#;
        let cfg: RouteFile = toml::from_str(toml).unwrap();
        assert_eq!(cfg.route.len(), 1);
        assert_eq!(cfg.route[0].match_pattern, "/users/**");
        assert_eq!(cfg.route[0].service, "user-service");
        assert!(!cfg.route[0].public);
    }
}
```

- [ ] **Step 2：运行确认编译失败**

Run: `cargo test -p api-gateway config::tests`
Expected: `cannot find type RouteFile`。

- [ ] **Step 3：实现 RouteFile 与扩展 RouteConfig**

重写 `services/api-gateway/src/config.rs`：

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct RouteConfig {
    #[serde(rename = "match")]
    pub match_pattern: String,
    pub service: String,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub rewrite: Vec<RewriteSpec>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum RewriteSpec {
    #[serde(rename = "strip_prefix")]
    StripPrefix { prefix: String },
    #[serde(rename = "redirect")]
    Redirect { status: u16, to: String },
    #[serde(rename = "header_set")]
    HeaderSet { name: String, value: String },
    #[serde(rename = "header_add")]
    HeaderAdd { name: String, value: String },
    #[serde(rename = "path_param")]
    PathParam { from: String, to: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct RouteFile {
    pub route: Vec<RouteConfig>,
}

#[derive(Debug, Clone, Default)]
pub struct RouteTable {
    pub routes: Vec<RouteConfig>,
}
```

- [ ] **Step 4：运行测试确认通过**

Run: `cargo test -p api-gateway config::tests`
Expected: 1 passed。

- [ ] **Step 5：提交**

```bash
git add services/api-gateway/src/config.rs
git commit -m "refactor(api-gateway): RouteConfig with match/rewrite, RouteTable"
```

---

## Task 4：实现 routes_loader.rs

**Files:**
- Create: `services/api-gateway/src/routes_loader.rs`

- [ ] **Step 1：写失败的测试**

`services/api-gateway/tests/routes_loader_test.rs`：

```rust
use api_gateway::routes_loader::load_routes;
use std::fs;
use tempfile::tempdir;

#[test]
fn loads_single_file() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("auth.router.toml"),
        r#"
        [[route]]
        match = "/auth/**"
        service = "auth-service"
        public = true
        "#,
    ).unwrap();
    let table = load_routes(dir.path()).unwrap();
    assert_eq!(table.routes.len(), 1);
    assert_eq!(table.routes[0].service, "auth-service");
}

#[test]
fn rejects_duplicate_match() {
    let dir = tempdir().unwrap();
    for name in &["a.router.toml", "b.router.toml"] {
        fs::write(
            dir.path().join(name),
            r#"
            [[route]]
            match = "/users/**"
            service = "user-service"
            "#,
        ).unwrap();
    }
    let err = load_routes(dir.path()).unwrap_err();
    assert!(err.to_string().contains("duplicate"));
}

#[test]
fn rejects_unknown_rewrite_type() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("bad.router.toml"),
        r#"
        [[route]]
        match = "/x/**"
        service = "x"
        rewrite = [{ type = "unknown_op", foo = "bar" }]
        "#,
    ).unwrap();
    assert!(load_routes(dir.path()).is_err());
}
```

`tempfile` 加入 `[dev-dependencies]`：

```toml
tempfile = "3"
```

- [ ] **Step 2：运行确认失败**

Run: `cargo test -p api-gateway --test routes_loader_test`
Expected: 编译错误 `module routes_loader not found`。

- [ ] **Step 3：实现 routes_loader.rs**

```rust
use crate::config::{RouteConfig, RouteTable, RouteFile};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("no .router.toml files in {0}")]
    Empty(String),
    #[error("parse {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("glob error: {0}")]
    Glob(#[from] glob::PatternError),
    #[error("duplicate match pattern: {0}")]
    Duplicate(String),
}

pub fn load_routes(dir: &Path) -> Result<RouteTable, LoadError> {
    let pattern = dir.join("*.router.toml");
    let mut entries: Vec<_> = glob::glob(pattern.to_str().unwrap())?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort();

    if entries.is_empty() {
        return Err(LoadError::Empty(dir.display().to_string()));
    }

    let mut routes = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for entry in entries {
        let content = std::fs::read_to_string(&entry)?;
        let file: RouteFile = toml::from_str(&content).map_err(|e| LoadError::Parse {
            path: entry.display().to_string(),
            source: e,
        })?;
        for r in file.route {
            if !seen.insert(r.match_pattern.clone()) {
                return Err(LoadError::Duplicate(r.match_pattern));
            }
            routes.push(r);
        }
    }
    Ok(RouteTable { routes })
}
```

- [ ] **Step 4：注册到 lib.rs**

修改 `services/api-gateway/src/lib.rs`：

```rust
pub mod routes_loader;
pub mod rewrite;
```

- [ ] **Step 5：运行测试确认通过**

Run: `cargo test -p api-gateway --test routes_loader_test`
Expected: 3 passed。

- [ ] **Step 6：提交**

```bash
git add services/api-gateway/src/routes_loader.rs services/api-gateway/src/lib.rs services/api-gateway/Cargo.toml services/api-gateway/tests/routes_loader_test.rs
git commit -m "feat(api-gateway): load routes/*.router.toml with conflict detection"
```

---

## Task 5：实现 rewrite.rs 规则执行器

**Files:**
- Create: `services/api-gateway/src/rewrite.rs`

- [ ] **Step 1：写失败的测试**

`services/api-gateway/tests/rewrite_test.rs`：

```rust
use api_gateway::config::RewriteSpec;
use api_gateway::rewrite::apply;

#[test]
fn strip_prefix_removes_prefix() {
    let rules = vec![RewriteSpec::StripPrefix { prefix: "/api".into() }];
    let mut req = axum::http::Request::builder()
        .uri("/api/users/1")
        .body(axum::body::Body::empty())
        .unwrap();
    apply(&rules, &mut req).unwrap();
    assert_eq!(req.uri().path(), "/users/1");
}

#[test]
fn header_set_overwrites() {
    let rules = vec![RewriteSpec::HeaderSet {
        name: "x-test".into(),
        value: "v".into(),
    }];
    let mut req = axum::http::Request::builder()
        .uri("/x")
        .header("x-test", "old")
        .body(axum::body::Body::empty())
        .unwrap();
    apply(&rules, &mut req).unwrap();
    assert_eq!(req.headers().get("x-test").unwrap(), "v");
}

#[test]
fn redirect_short_circuits() {
    let rules = vec![
        RewriteSpec::HeaderSet { name: "x-a".into(), value: "1".into() },
        RewriteSpec::Redirect { status: 301, to: "/new".into() },
    ];
    let mut req = axum::http::Request::builder()
        .uri("/old")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = apply(&rules, &mut req).unwrap_err();
    assert_eq!(resp.status(), axum::http::StatusCode::MOVED_PERMANENTLY);
    assert_eq!(resp.headers().get("location").unwrap(), "/new");
}
```

- [ ] **Step 2：运行确认失败**

Run: `cargo test -p api-gateway --test rewrite_test`
Expected: `module rewrite not found`。

- [ ] **Step 3：实现 rewrite.rs**

```rust
use crate::config::RewriteSpec;
use axum::body::Body;
use axum::http::{Request, Response, StatusCode, Uri};
use axum::response::IntoResponse;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RewriteError {
    #[error("prefix not in path: {0}")]
    PrefixMissing(String),
    #[error("invalid uri: {0}")]
    InvalidUri(String),
}

pub fn apply(rules: &[RewriteSpec], req: &mut Request<Body>) -> Result<(), Response> {
    for rule in rules {
        match rule {
            RewriteSpec::StripPrefix { prefix } => {
                let p = req.uri().path();
                if let Some(rest) = p.strip_prefix(prefix.as_str()) {
                    let new_path = if rest.is_empty() { "/" } else { rest };
                    let new_uri = build_uri(req.uri(), new_path);
                    *req.uri_mut() = new_uri;
                } else {
                    return Err(RewriteError::PrefixMissing(prefix.clone()).into_response());
                }
            }
            RewriteSpec::HeaderSet { name, value } => {
                req.headers_mut().insert(
                    name.parse().unwrap(),
                    value.parse().unwrap(),
                );
            }
            RewriteSpec::HeaderAdd { name, value } => {
                req.headers_mut().append(
                    name.parse().unwrap(),
                    value.parse().unwrap(),
                );
            }
            RewriteSpec::PathParam { from, to } => {
                let mut parts = req.uri().clone().into_parts();
                let path = parts.path_and_query.as_mut().unwrap();
                let new_path = path.path().replace(from, to);
                *path = new_path.try_into().unwrap();
                *req.uri_mut() = Uri::from_parts(parts).unwrap();
            }
            RewriteSpec::Redirect { status, to } => {
                let s = StatusCode::from_u16(*status)
                    .unwrap_or(StatusCode::MOVED_PERMANENTLY);
                let resp = Response::builder()
                    .status(s)
                    .header("location", to)
                    .body(Body::empty())
                    .unwrap();
                return Err(resp);
            }
        }
    }
    Ok(())
}

fn build_uri(orig: &Uri, new_path: &str) -> Uri {
    let mut parts = orig.clone().into_parts();
    let pq = parts.path_and_query.get_or_insert_with(|| "/".parse().unwrap());
    *pq = new_path.parse().unwrap();
    Uri::from_parts(parts).unwrap()
}
```

- [ ] **Step 4：运行测试确认通过**

Run: `cargo test -p api-gateway --test rewrite_test`
Expected: 3 passed。

- [ ] **Step 5：提交**

```bash
git add services/api-gateway/src/rewrite.rs services/api-gateway/tests/rewrite_test.rs
git commit -m "feat(api-gateway): rewrite rule chain executor"
```

---

## Task 6：改造 AppState 使用 ArcSwap<RouteTable>

**Files:**
- Modify: `services/api-gateway/src/lib.rs`

- [ ] **Step 1：写测试**

在 `services/api-gateway/src/lib.rs` 末尾：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn route_table_swap() {
        let swap = arc_swap::ArcSwap::from_pointee(RouteTable::default());
        swap.store(Arc::new(RouteTable {
            routes: vec![RouteConfig {
                match_pattern: "/a".into(),
                service: "s".into(),
                public: false,
                rewrite: vec![],
            }],
        }));
        let cur = swap.load();
        assert_eq!(cur.routes.len(), 1);
    }
}
```

- [ ] **Step 2：运行确认编译失败**

Run: `cargo test -p api-gateway lib::tests`
Expected: `cannot find type RouteTable` 或 `ArcSwap`。

- [ ] **Step 3：修改 lib.rs**

```rust
pub mod blacklist;
pub mod config;
pub mod discovery;
pub mod jwt;
pub mod middleware;
pub mod rewrite;
pub mod routes_loader;
pub mod router;

pub use config::{GatewayConfig, RewriteSpec, RouteConfig, RouteFile, RouteTable};
pub use discovery::{ServiceDiscovery, ServiceInstance};
pub use jwt::{JwtClaims, JwtValidator};
pub use router::RouterConfig;

use axum::{routing::get, Router};
use redis::aio::MultiplexedConnection;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub jwt_validator: Arc<JwtValidator>,
    pub redis_conn: MultiplexedConnection,
    pub redis_prefix: String,
    pub router_cfg: Arc<RouterConfig>,
    pub route_table: arc_swap::ArcSwap<RouteTable>,
}

pub async fn create_app(state: AppState) -> Router {
    let health_route = Router::new().route(
        "/health",
        get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
    );
    Router::new().merge(health_route).with_state(Arc::new(state))
}

pub async fn create_test_app() -> Router {
    let health_route = Router::new().route(
        "/health",
        get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
    );
    Router::new().merge(health_route).with_state(Arc::new(()))
}
```

- [ ] **Step 4：运行测试确认通过**

Run: `cargo test -p api-gateway lib::tests`
Expected: 1 passed。

- [ ] **Step 5：提交**

```bash
git add services/api-gateway/src/lib.rs
git commit -m "refactor(api-gateway): AppState routes via ArcSwap<RouteTable>"
```

---

## Task 7：路由匹配与转发接入重写

**Files:**
- Modify: `services/api-gateway/src/router.rs`
- Modify: `services/api-gateway/src/main.rs`

- [ ] **Step 1：写测试**

`services/api-gateway/src/router.rs` 末尾：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RouteConfig;

    #[test]
    fn match_wildcard_prefix() {
        let r = RouteConfig {
            match_pattern: "/auth/**".into(),
            service: "auth".into(),
            public: true,
            rewrite: vec![],
        };
        assert!(super::match_route("/api/auth/login", "/auth/**"));
        assert!(!super::match_route("/api/users/1", "/auth/**"));
    }
}
```

`main.rs` 删掉旧的 `find_route` 与 `match_prefix`，在 `router.rs` 暴露：

```rust
pub fn match_route(path: &str, pattern: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        let api_prefix = format!("/api{}", prefix);
        path == api_prefix || path.starts_with(&format!("{}/", api_prefix))
    } else {
        path == format!("/api{}", pattern)
    }
}
```

`router::forward_to_service` 增加 `rewrite` 参数（从 AppState 取），调用 `rewrite::apply`：

```rust
pub async fn forward_to_service(
    cfg: Arc<RouterConfig>,
    mut request: Request<Body>,
    service_name: &str,
    rewrites: &[crate::config::RewriteSpec],
) -> Response {
    if let Err(resp) = crate::rewrite::apply(rewrites, &mut request) {
        return resp;
    }

    let instance = match cfg.discovery.discover(service_name).await {
        Ok(i) => i,
        Err(status) => return status.into_response(),
    };

    let target_url = format!(
        "http://{}:{}{}",
        instance.address,
        instance.port,
        request.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or("")
    );

    let (parts, body) = request.into_parts();
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let req_builder = cfg.http_client.request(parts.method.clone(), &target_url);
    let resp = match req_builder.headers(parts.headers.clone()).body(bytes).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("forward to {} failed: {:?}", service_name, e);
            return (StatusCode::BAD_GATEWAY, "upstream error").into_response();
        }
    };

    let status = resp.status();
    let headers = resp.headers().clone();
    let body_bytes = resp.bytes().await.unwrap_or_default();

    let mut builder = Response::builder().status(status);
    for (key, value) in headers.iter() {
        let key_str = key.as_str();
        if !["transfer-encoding", "connection"].contains(&key_str) {
            builder = builder.header(key_str, value.as_bytes());
        }
    }
    builder.body(Body::from(body_bytes)).unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
```

- [ ] **Step 2：更新 main.rs 调用**

`proxy_handler` 改为：

```rust
async fn proxy_handler(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Response {
    let path = request.uri().path().to_string();
    let table = state.route_table.load();
    let route = match table.routes.iter().find(|r| router::match_route(&path, &r.match_pattern)) {
        Some(r) => r,
        None => return (StatusCode::NOT_FOUND, "unknown service").into_response(),
    };
    router::forward_to_service(
        state.router_cfg.clone(),
        request,
        &route.service,
        &route.rewrite,
    ).await
}
```

- [ ] **Step 3：运行测试**

Run: `cargo test -p api-gateway router::tests`
Expected: 1 passed。

- [ ] **Step 4：提交**

```bash
git add services/api-gateway/src/router.rs services/api-gateway/src/main.rs
git commit -m "feat(api-gateway): match wildcards and apply rewrite before forwarding"
```

---

## Task 8：CLI 子命令 validate / serve

**Files:**
- Create: `services/api-gateway/src/cli.rs`

- [ ] **Step 1：写测试**

`services/api-gateway/tests/validate_cli_test.rs`：

```rust
use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn validate_exits_zero_on_valid() {
    let dir = tempdir().unwrap();
    let routes = dir.path().join("routes");
    fs::create_dir(&routes).unwrap();
    fs::write(
        routes.join("a.router.toml"),
        r#"[[route]]
        match = "/a/**"
        service = "a"
        "#,
    ).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_api-gateway"))
        .args(["validate", "--routes"])
        .arg(&routes)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn validate_exits_nonzero_on_duplicate() {
    let dir = tempdir().unwrap();
    let routes = dir.path().join("routes");
    fs::create_dir(&routes).unwrap();
    for n in &["a.router.toml", "b.router.toml"] {
        fs::write(
            routes.join(n),
            r#"[[route]]
            match = "/x/**"
            service = "x"
            "#,
        ).unwrap();
    }
    let out = Command::new(env!("CARGO_BIN_EXE_api-gateway"))
        .args(["validate", "--routes"])
        .arg(&routes)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("duplicate"));
}
```

- [ ] **Step 2：实现 cli.rs**

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "api-gateway")]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Parse and check routes/*.router.toml; exit 0 on success.
    Validate {
        #[arg(long, default_value = "config/api-gateway/routes")]
        routes: PathBuf,
    },
    /// Run the gateway.
    Serve {
        #[arg(long, default_value = "config/api-gateway/routes")]
        routes: PathBuf,
    },
}
```

- [ ] **Step 3：main.rs 接入 CLI**

```rust
mod cli;
use clap::Parser;
use api_gateway::routes_loader::load_routes;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "api_gateway=debug,info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = cli::Cli::parse();
    match cli.cmd {
        cli::Cmd::Validate { routes } => match load_routes(&routes) {
            Ok(t) => {
                println!("OK: {} routes", t.routes.len());
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("FAIL: {e}");
                std::process::exit(1);
            }
        },
        cli::Cmd::Serve { routes } => serve(routes).await,
    }
}

async fn serve(routes_dir: PathBuf) {
    let table = load_routes(&routes_dir).expect("routes load failed");
    // ... 读取 GatewayConfig、构建 AppState、ArcSwap.store ...
    // ... 注册 SIGHUP handler（Task 9） ...
    // ... axum::serve ...
}
```

- [ ] **Step 4：运行测试**

Run: `cargo test -p api-gateway --test validate_cli_test`
Expected: 2 passed。

- [ ] **Step 5：提交**

```bash
git add services/api-gateway/src/cli.rs services/api-gateway/src/main.rs services/api-gateway/tests/validate_cli_test.rs
git commit -m "feat(api-gateway): cli validate/serve subcommands"
```

---

## Task 9：reload 命令与 SIGHUP handler

**Files:**
- Modify: `services/api-gateway/src/main.rs`
- Create: `services/api-gateway/tests/reload_test.rs`

- [ ] **Step 1：写测试**

```rust
// reload_test.rs 简化为 unit test
#[tokio::test]
async fn reload_swaps_table_on_valid() {
    use api_gateway::config::RouteTable;
    use arc_swap::ArcSwap;
    use std::sync::Arc;

    let swap = ArcSwap::from_pointee(RouteTable::default());
    let new_table = RouteTable {
        routes: vec![api_gateway::config::RouteConfig {
            match_pattern: "/x/**".into(),
            service: "x".into(),
            public: false,
            rewrite: vec![],
        }],
    };
    swap.store(Arc::new(new_table));
    assert_eq!(swap.load().routes.len(), 1);
}
```

- [ ] **Step 2：main.rs 注册 SIGHUP + admin endpoint**

```rust
async fn serve(routes_dir: PathBuf) {
    let cfg = load_global_config();

    let table = load_routes(&routes_dir).expect("routes load failed");
    let route_table = arc_swap::ArcSwap::from_pointee(table);

    // SIGHUP handler (POSIX only; Windows 跳过)
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sighup = signal(SignalKind::hangup()).unwrap();
        let rt = route_table.clone();
        let dir = routes_dir.clone();
        tokio::spawn(async move {
            while sighup.recv().await.is_some() {
                match reload(&dir, &rt) {
                    Ok(n) => tracing::info!("reload ok, {} routes", n),
                    Err(e) => tracing::error!("reload failed: {e}"),
                }
            }
        });
    }

    let state = build_app_state(&cfg, route_table.clone());

    // Admin route
    let admin = Router::new().route(
        "/__admin/reload",
        post({
            let rt = route_table.clone();
            let dir = routes_dir.clone();
            move || async move {
                match reload(&dir, &rt) {
                    Ok(n) => (axum::http::StatusCode::OK, format!("reloaded: {} routes", n)).into_response(),
                    Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
                }
            }
        }),
    );

    let app = Router::new().merge(admin).merge(build_proxy_router(state));
    let listener = tokio::net::TcpListener::bind(&cfg.listen_addr()).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn reload(dir: &Path, swap: &arc_swap::ArcSwap<RouteTable>) -> Result<usize, api_gateway::routes_loader::LoadError> {
    let t = load_routes(dir)?;
    let n = t.routes.len();
    swap.store(Arc::new(t));
    Ok(n)
}
```

`load_global_config` / `build_app_state` / `build_proxy_router` 在 Task 8 已经搭好骨架，本 Task 只补 reload 与 admin 端点。

`Serve` 同时注册 `POST /__admin/reload`。

- [ ] **Step 3：运行测试**

Run: `cargo test -p api-gateway --test reload_test`
Expected: 1 passed。

- [ ] **Step 4：提交**

```bash
git add services/api-gateway/src/main.rs services/api-gateway/tests/reload_test.rs
git commit -m "feat(api-gateway): SIGHUP + /__admin/reload swap route table"
```

---

## Task 10：全量回归

**Files:** none

- [ ] **Step 1：跑全套测试**

Run: `cargo test -p api-gateway`
Expected: 全部通过。

- [ ] **Step 2：本地起 gateway**

Run: `cargo run -p api-gateway -- serve --routes docker/config/api-gateway/routes`
Expected: 监听 3000，启动日志打印 `5 routes`。

- [ ] **Step 3：curl 烟测**

Run: `curl -i http://localhost:3000/health`
Expected: `200 OK`。

- [ ] **Step 4：commit & 收尾**

```bash
git status
# 若有遗留格式改动：
git add -A
git commit -m "chore: post-impl cleanup"
```
