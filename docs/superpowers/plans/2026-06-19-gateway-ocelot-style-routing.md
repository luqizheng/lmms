# API Gateway Ocelot 风格路由 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 api-gateway 路由配置由 `match` + `rewrite[]` 重构为 Ocelot 风格的 `upstream` / `downstream` 模板，全部 router.toml 用新写法重写。

**Architecture:** 配置反序列化得到 `RouteConfig { upstream, downstream, service, public, ws, headers }`；启动期编译为 `CompiledRoute`（由 `Template { parts: Vec<Literal|Var> }` 表示 upstream/downstream）；请求期对 path+query 做"提取 dict → 渲染 downstream"两步，再透传给 Consul 发现的微服务。

**Tech Stack:** Rust 2021 / axum / serde / toml / arc_swap，仍走 musl 容器，cargo 命令在 PowerShell 真实环境跑（沙箱不支持）。

---

## 文件结构

| 文件 | 责任 |
|---|---|
| `services/api-gateway/src/template.rs`（新增） | `Template` / `TplPart` 类型、`parse()` / `extract()` / `render()` 三个纯函数 |
| `services/api-gateway/src/config.rs` | `RouteConfig` 结构改为新字段，删除 `RewriteSpec` |
| `services/api-gateway/src/routes_loader.rs` | 加载 → 编译 → 校验，输出 `Vec<CompiledRoute>` |
| `services/api-gateway/src/rewrite.rs` | 删除全部内容（Ocelot 模式不再需要 strip_prefix 等） |
| `services/api-gateway/src/router.rs` | 接收已渲染的 `target_path_query`，按 service 转发 |
| `services/api-gateway/src/main.rs` | `proxy_handler` 改用 `extract → render → forward` |
| `services/api-gateway/src/lib.rs` | 模块声明同步、re-export 调整 |
| `services/api-gateway/src/cli.rs` | 注释/帮助文案同步 |
| `services/api-gateway/tests/template_test.rs`（新增） | template 单元测试 |
| `services/api-gateway/tests/routes_loader_test.rs` | 改写为新模型 |
| `services/api-gateway/tests/rewrite_test.rs` | 删除（替换为 template_test） |
| `services/api-gateway/tests/integration_test.rs` | 修匹配/转发断言 |
| `docker/config/api-gateway/routes/*.router.toml` | 8 个文件全部改写 |
| `.trae/rules/routing-rules.md` | §2.2/§2.3/§2.5/§2.6/§2.7 重写 |
| `.trae/rules/project_rules.md` | 删除 rewrite 类型白名单段落 |
| `docs/guides/api-gateway-routing.md` | 同步用户文档（如已存在） |

---

## Task 1：新增 `template.rs` 数据结构与解析

**Files:**
- Create: `services/api-gateway/src/template.rs`
- Modify: `services/api-gateway/src/lib.rs`
- Test: `services/api-gateway/tests/template_test.rs`

- [ ] **Step 1: 写失败的解析单元测试**

写到 `services/api-gateway/src/template.rs` 末尾的 `#[cfg(test)] mod tests`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_literal_only() {
        let t = Template::parse("/iam/auth/login").unwrap();
        assert_eq!(t.parts, vec![TplPart::Literal("/iam/auth/login".into())]);
    }

    #[test]
    fn parse_one_var_at_tail() {
        let t = Template::parse("/v1/iam/{rest}").unwrap();
        assert_eq!(
            t.parts,
            vec![
                TplPart::Literal("/v1/iam/".into()),
                TplPart::Var("rest".into()),
            ]
        );
    }

    #[test]
    fn parse_multi_vars_with_separator() {
        let t = Template::parse("/a/{x}/b/{y}").unwrap();
        assert_eq!(
            t.parts,
            vec![
                TplPart::Literal("/a/".into()),
                TplPart::Var("x".into()),
                TplPart::Literal("/b/".into()),
                TplPart::Var("y".into()),
            ]
        );
    }

    #[test]
    fn parse_rejects_adjacent_vars() {
        let err = Template::parse("/a/{x}{y}").unwrap_err();
        assert!(matches!(err, TemplateError::AdjacentVars));
    }

    #[test]
    fn parse_rejects_unclosed_brace() {
        assert!(matches!(
            Template::parse("/a/{x").unwrap_err(),
            TemplateError::UnclosedBrace
        ));
    }

    #[test]
    fn parse_rejects_empty_var_name() {
        assert!(matches!(
            Template::parse("/a/{}/b").unwrap_err(),
            TemplateError::EmptyVarName
        ));
    }

    #[test]
    fn parse_rejects_duplicate_var_name() {
        assert!(matches!(
            Template::parse("/a/{x}/b/{x}").unwrap_err(),
            TemplateError::DuplicateVar(_)
        ));
    }
}
```

- [ ] **Step 2: 实现最小 `template.rs`**

```rust
//! Ocelot 风格路径模板：字面量与 {var} 交替，提取 dict 后做朴素 replace。

use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TplPart {
    Literal(String),
    Var(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    pub parts: Vec<TplPart>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TemplateError {
    #[error("unclosed '{{' in template")]
    UnclosedBrace,
    #[error("empty variable name")]
    EmptyVarName,
    #[error("two variables must be separated by a non-empty literal")]
    AdjacentVars,
    #[error("duplicate variable name: {0}")]
    DuplicateVar(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExtractError {
    #[error("literal '{0}' not matched at cursor {1}")]
    LiteralMismatch(String, usize),
    #[error("anchor literal '{0}' not found after variable")]
    AnchorNotFound(String),
    #[error("trailing input after template consumed")]
    TrailingInput,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RenderError {
    #[error("missing variable in dict: {0}")]
    MissingVar(String),
}

impl Template {
    pub fn parse(input: &str) -> Result<Self, TemplateError> {
        let bytes = input.as_bytes();
        let mut parts: Vec<TplPart> = Vec::new();
        let mut buf = String::new();
        let mut i = 0;
        let mut seen: HashSet<String> = HashSet::new();
        while i < bytes.len() {
            let c = bytes[i] as char;
            if c == '{' {
                if !buf.is_empty() {
                    parts.push(TplPart::Literal(std::mem::take(&mut buf)));
                } else if matches!(parts.last(), Some(TplPart::Var(_))) {
                    return Err(TemplateError::AdjacentVars);
                }
                let end = input[i + 1..]
                    .find('}')
                    .ok_or(TemplateError::UnclosedBrace)?
                    + i
                    + 1;
                let name = &input[i + 1..end];
                if name.is_empty() {
                    return Err(TemplateError::EmptyVarName);
                }
                if !seen.insert(name.to_string()) {
                    return Err(TemplateError::DuplicateVar(name.to_string()));
                }
                parts.push(TplPart::Var(name.to_string()));
                i = end + 1;
            } else {
                buf.push(c);
                i += 1;
            }
        }
        if !buf.is_empty() {
            parts.push(TplPart::Literal(buf));
        }
        Ok(Self { parts })
    }

    pub fn var_names(&self) -> Vec<String> {
        self.parts
            .iter()
            .filter_map(|p| match p {
                TplPart::Var(n) => Some(n.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn extract(&self, input: &str) -> Result<HashMap<String, String>, ExtractError> {
        let mut dict = HashMap::new();
        let mut cursor = 0usize;
        for (idx, part) in self.parts.iter().enumerate() {
            match part {
                TplPart::Literal(s) => {
                    if !input[cursor..].starts_with(s.as_str()) {
                        return Err(ExtractError::LiteralMismatch(s.clone(), cursor));
                    }
                    cursor += s.len();
                }
                TplPart::Var(name) => {
                    let next_lit = self.parts[idx + 1..].iter().find_map(|p| match p {
                        TplPart::Literal(s) => Some(s.as_str()),
                        _ => None,
                    });
                    let end = match next_lit {
                        Some(lit) => input[cursor..]
                            .find(lit)
                            .map(|off| cursor + off)
                            .ok_or_else(|| ExtractError::AnchorNotFound(lit.to_string()))?,
                        None => input.len(),
                    };
                    dict.insert(name.clone(), input[cursor..end].to_string());
                    cursor = end;
                }
            }
        }
        if cursor != input.len() {
            return Err(ExtractError::TrailingInput);
        }
        Ok(dict)
    }

    pub fn render(&self, dict: &HashMap<String, String>) -> Result<String, RenderError> {
        let mut out = String::new();
        for p in &self.parts {
            match p {
                TplPart::Literal(s) => out.push_str(s),
                TplPart::Var(n) => {
                    let v = dict
                        .get(n)
                        .ok_or_else(|| RenderError::MissingVar(n.clone()))?;
                    out.push_str(v);
                }
            }
        }
        Ok(out)
    }
}
```

在 `lib.rs` 加：

```rust
pub mod template;
```

- [ ] **Step 3: 跑解析测试，全部 PASS**

在 PowerShell 真实环境（vcvarsall x64）：

```powershell
cargo test -p api-gateway --lib template::tests
```

期望：parse 七个 case 全 PASS。

- [ ] **Step 4: 写 extract 单元测试到 `tests/template_test.rs`**

```rust
use api_gateway::template::{ExtractError, Template};

#[test]
fn extract_one_var_tail() {
    let t = Template::parse("/v1/iam/{rest}").unwrap();
    let d = t.extract("/v1/iam/users/42").unwrap();
    assert_eq!(d["rest"], "users/42");
}

#[test]
fn extract_multi_vars() {
    let t = Template::parse("/a/{x}/b/{y}").unwrap();
    let d = t.extract("/a/foo/b/bar").unwrap();
    assert_eq!(d["x"], "foo");
    assert_eq!(d["y"], "bar");
}

#[test]
fn extract_var_value_can_contain_slash() {
    let t = Template::parse("/v1/iam/{rest}").unwrap();
    let d = t.extract("/v1/iam/a/b/c").unwrap();
    assert_eq!(d["rest"], "a/b/c");
}

#[test]
fn extract_literal_mismatch() {
    let t = Template::parse("/v1/iam/{rest}").unwrap();
    let err = t.extract("/v2/iam/x").unwrap_err();
    assert!(matches!(err, ExtractError::LiteralMismatch(_, _)));
}

#[test]
fn extract_anchor_not_found() {
    let t = Template::parse("/a/{x}/b/{y}").unwrap();
    let err = t.extract("/a/foo/no-b-here").unwrap_err();
    assert!(matches!(err, ExtractError::AnchorNotFound(_)));
}
```

- [ ] **Step 5: 写 render 单元测试**

继续追加：

```rust
use std::collections::HashMap;
use api_gateway::template::RenderError;

#[test]
fn render_path_with_vars() {
    let t = Template::parse("/iam/{rest}").unwrap();
    let mut d = HashMap::new();
    d.insert("rest".to_string(), "users/42".to_string());
    assert_eq!(t.render(&d).unwrap(), "/iam/users/42");
}

#[test]
fn render_with_query() {
    let t = Template::parse("/billing/subs/{sub}/updates?unitId={unit}").unwrap();
    let mut d = HashMap::new();
    d.insert("sub".to_string(), "s1".to_string());
    d.insert("unit".to_string(), "u9".to_string());
    assert_eq!(
        t.render(&d).unwrap(),
        "/billing/subs/s1/updates?unitId=u9"
    );
}

#[test]
fn render_missing_var() {
    let t = Template::parse("/a/{x}").unwrap();
    let d = HashMap::new();
    assert!(matches!(
        t.render(&d).unwrap_err(),
        RenderError::MissingVar(_)
    ));
}
```

- [ ] **Step 6: 跑全部 template 测试**

```powershell
cargo test -p api-gateway template
```

期望：全部 PASS。

- [ ] **Step 7: Commit**

```powershell
git add services/api-gateway/src/template.rs services/api-gateway/src/lib.rs services/api-gateway/tests/template_test.rs
git commit -m "feat(api-gateway): add Ocelot-style path template (parse/extract/render)"
```

---

## Task 2：替换 `RouteConfig` 与删除 `RewriteSpec`

**Files:**
- Modify: `services/api-gateway/src/config.rs`
- Modify: `services/api-gateway/src/lib.rs`

- [ ] **Step 1: 改写 `config.rs`**

把 `RouteConfig` / `RewriteSpec` 段落替换为：

```rust
use common::config::{RedisConfig, ServerConfig};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct RouteConfig {
    pub upstream: String,
    pub downstream: String,
    pub service: String,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub ws: bool,
    #[serde(default)]
    pub headers: HashMap<String, String>,
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

`GatewayConfig` 段落保持不变（仅 `routes: Vec<RouteConfig>` 沿用新结构）。删除 `RewriteSpec` 枚举与所有引用。

- [ ] **Step 2: 删除 `lib.rs` 中的 `RewriteSpec` re-export**

把：

```rust
pub use config::{GatewayConfig, RewriteSpec, RouteConfig, RouteFile, RouteTable};
```

改为：

```rust
pub use config::{GatewayConfig, RouteConfig, RouteFile, RouteTable};
```

也把 `lib.rs` 末尾测试里的 `RouteConfig { match_pattern, ..., rewrite }` 改为新字段：

```rust
swap.store(Arc::new(RouteTable {
    routes: vec![RouteConfig {
        upstream: "/v1/a".into(),
        downstream: "/a".into(),
        service: "s".into(),
        public: false,
        ws: false,
        headers: Default::default(),
    }],
}));
```

- [ ] **Step 3: `cargo check` 看断面**

```powershell
cargo check -p api-gateway
```

期望：在 `routes_loader.rs` / `main.rs` / `rewrite.rs` 等处出错（这些将在后续 Task 修）。先记录错误数量。

- [ ] **Step 4: Commit（带 broken state）**

允许此处中间态不可编译，下游 Task 立即接续：

```powershell
git add services/api-gateway/src/config.rs services/api-gateway/src/lib.rs
git commit -m "refactor(api-gateway): replace RouteConfig with upstream/downstream schema"
```

---

## Task 3：删除 `rewrite.rs`，`router.rs` 接收已渲染的 `target_path_query`

**Files:**
- Delete: `services/api-gateway/src/rewrite.rs`
- Delete: `services/api-gateway/tests/rewrite_test.rs`
- Modify: `services/api-gateway/src/router.rs`
- Modify: `services/api-gateway/src/lib.rs`

- [ ] **Step 1: 删除 `rewrite.rs` 与对应测试**

```powershell
git rm services/api-gateway/src/rewrite.rs services/api-gateway/tests/rewrite_test.rs
```

- [ ] **Step 2: `lib.rs` 删除 `pub mod rewrite;`**

- [ ] **Step 3: 改写 `router.rs::forward_to_service` 为接收目标 path+query**

替换函数签名与 url 拼装：

```rust
pub async fn forward_to_service(
    cfg: Arc<RouterConfig>,
    request: Request<Body>,
    service_name: &str,
    target_path_query: &str,
) -> Response {
    let instance = match cfg.discovery.discover(service_name).await {
        Ok(i) => i,
        Err(status) => return status.into_response(),
    };

    let target_url = format!(
        "http://{}:{}{}",
        instance.address, instance.port, target_path_query
    );

    let (parts, body) = request.into_parts();
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let req_builder = cfg.http_client.request(parts.method.clone(), &target_url);

    let resp = match req_builder
        .headers(parts.headers.clone())
        .body(bytes)
        .send()
        .await
    {
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

    builder
        .body(Body::from(body_bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
```

- [ ] **Step 4: Commit**

```powershell
git add -A
git commit -m "refactor(api-gateway): drop rewrite module; pass rendered target path to forwarder"
```

---

## Task 4：`routes_loader` 增加编译与校验

**Files:**
- Modify: `services/api-gateway/src/routes_loader.rs`
- Modify: `services/api-gateway/src/lib.rs`
- Test: `services/api-gateway/tests/routes_loader_test.rs`

- [ ] **Step 1: 写失败的加载测试**

替换 `routes_loader_test.rs` 全部内容：

```rust
use api_gateway::routes_loader::{load_routes, LoadError};
use std::fs;
use tempfile::tempdir;

fn write(dir: &std::path::Path, name: &str, body: &str) {
    fs::write(dir.join(name), body).unwrap();
}

#[test]
fn loads_basic_route() {
    let d = tempdir().unwrap();
    write(
        d.path(),
        "iam.router.toml",
        r#"
[[route]]
upstream   = "/v1/iam/auth/login"
downstream = "/iam/auth/login"
service    = "iam-service"
public     = true
"#,
    );
    let table = load_routes(d.path()).unwrap();
    assert_eq!(table.routes.len(), 1);
    assert_eq!(table.routes[0].upstream, "/v1/iam/auth/login");
}

#[test]
fn rejects_var_mismatch() {
    let d = tempdir().unwrap();
    write(
        d.path(),
        "x.router.toml",
        r#"
[[route]]
upstream   = "/v1/iam/{rest}"
downstream = "/iam/{other}"
service    = "iam-service"
"#,
    );
    let err = load_routes(d.path()).unwrap_err();
    assert!(matches!(err, LoadError::VarMismatch { .. }));
}

#[test]
fn rejects_duplicate_upstream() {
    let d = tempdir().unwrap();
    write(
        d.path(),
        "x.router.toml",
        r#"
[[route]]
upstream   = "/v1/a"
downstream = "/a"
service    = "s"

[[route]]
upstream   = "/v1/a"
downstream = "/a"
service    = "s"
"#,
    );
    let err = load_routes(d.path()).unwrap_err();
    assert!(matches!(err, LoadError::DuplicateUpstream(_)));
}

#[test]
fn rejects_legacy_match_field() {
    let d = tempdir().unwrap();
    write(
        d.path(),
        "x.router.toml",
        r#"
[[route]]
match   = "/v1/legacy"
service = "s"
"#,
    );
    let err = load_routes(d.path()).unwrap_err();
    assert!(matches!(err, LoadError::Parse { .. }));
}
```

> 注：测试需要 `tempfile`。若 `services/api-gateway/Cargo.toml` 的 `[dev-dependencies]` 未含 `tempfile`，加 `tempfile = "3"`。

- [ ] **Step 2: 改写 `routes_loader.rs`**

```rust
use crate::config::{RouteFile, RouteTable};
use crate::template::{Template, TemplateError};
use std::collections::HashSet;
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
    #[error("duplicate upstream: {0}")]
    DuplicateUpstream(String),
    #[error("template error in {field} '{template}': {source}")]
    Template {
        field: &'static str,
        template: String,
        #[source]
        source: TemplateError,
    },
    #[error("variable name set mismatch in route '{upstream}': upstream={up:?} downstream={down:?}")]
    VarMismatch {
        upstream: String,
        up: Vec<String>,
        down: Vec<String>,
    },
}

pub fn load_routes(dir: &Path) -> Result<RouteTable, LoadError> {
    let pattern = dir.join("*.router.toml");
    let pattern_str = pattern
        .to_str()
        .ok_or_else(|| LoadError::Empty(dir.display().to_string()))?;

    let mut entries: Vec<_> = glob::glob(pattern_str)?.filter_map(|e| e.ok()).collect();
    entries.sort();

    let mut routes = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for entry in entries {
        let content = std::fs::read_to_string(&entry)?;
        let file: RouteFile = toml::from_str(&content).map_err(|e| LoadError::Parse {
            path: entry.display().to_string(),
            source: e,
        })?;
        for r in file.route {
            // 编译期校验 upstream/downstream
            let up = Template::parse(&r.upstream).map_err(|e| LoadError::Template {
                field: "upstream",
                template: r.upstream.clone(),
                source: e,
            })?;
            let down = Template::parse(&r.downstream).map_err(|e| LoadError::Template {
                field: "downstream",
                template: r.downstream.clone(),
                source: e,
            })?;
            let mut up_vars = up.var_names();
            let mut down_vars = down.var_names();
            up_vars.sort();
            down_vars.sort();
            if up_vars != down_vars {
                return Err(LoadError::VarMismatch {
                    upstream: r.upstream.clone(),
                    up: up_vars,
                    down: down_vars,
                });
            }
            if !seen.insert(r.upstream.clone()) {
                return Err(LoadError::DuplicateUpstream(r.upstream));
            }
            routes.push(r);
        }
    }

    // 字面量长度降序：更具体的优先匹配
    routes.sort_by(|a, b| literal_len(&b.upstream).cmp(&literal_len(&a.upstream)));

    Ok(RouteTable { routes })
}

fn literal_len(template: &str) -> usize {
    let mut depth = 0;
    let mut n = 0;
    for c in template.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ if depth == 0 => n += 1,
            _ => {}
        }
    }
    n
}
```

- [ ] **Step 3: 跑加载测试**

```powershell
cargo test -p api-gateway --test routes_loader_test
```

期望：四个 case 全 PASS。

- [ ] **Step 4: Commit**

```powershell
git add services/api-gateway/src/routes_loader.rs services/api-gateway/tests/routes_loader_test.rs services/api-gateway/Cargo.toml
git commit -m "feat(api-gateway): compile and validate upstream/downstream templates at load time"
```

---

## Task 5：`main.rs` 改用 extract+render+forward

**Files:**
- Modify: `services/api-gateway/src/main.rs`

- [ ] **Step 1: 删除旧 `match_pattern` / `find_route` / `print_routes` 实现**

替换为：

```rust
use api_gateway::template::Template;
use axum::http::HeaderName;
use axum::http::HeaderValue;

fn print_routes(table: &RouteTable) {
    println!("routes:");
    for r in &table.routes {
        println!(
            "  - {}  ->  {} ({})  =>  {}",
            r.upstream,
            r.service,
            if r.public { "public" } else { "protected" },
            r.downstream,
        );
    }
}

fn find_and_render(
    routes: &[RouteConfig],
    path_and_query: &str,
) -> Option<(RouteConfig, String)> {
    for r in routes {
        let up = Template::parse(&r.upstream).ok()?;
        let dict = match up.extract(path_and_query) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let down = Template::parse(&r.downstream).ok()?;
        let target = down.render(&dict).ok()?;
        return Some((r.clone(), target));
    }
    None
}
```

- [ ] **Step 2: 改写 `proxy_handler`**

```rust
async fn proxy_handler(State(state): State<Arc<AppState>>, mut request: Request) -> Response {
    let pq = request
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());
    let table = state.route_table.load();

    let (route, target) = match find_and_render(&table.routes, &pq) {
        Some(x) => x,
        None => {
            tracing::warn!("no route matched for path: {}", pq);
            return (StatusCode::NOT_FOUND, "unknown service").into_response();
        }
    };

    // 注入 headers（启动期已校验合法，但运行期同样要 graceful）
    for (k, v) in &route.headers {
        if let (Ok(name), Ok(val)) = (k.parse::<HeaderName>(), v.parse::<HeaderValue>()) {
            request.headers_mut().insert(name, val);
        }
    }

    router::forward_to_service(state.router_cfg.clone(), request, &route.service, &target)
        .await
}
```

- [ ] **Step 3: `cargo check` 全绿**

```powershell
cargo check -p api-gateway
```

期望：编译通过。如有遗留对 `rewrite::apply` / `RewriteSpec` 的引用，删干净。

- [ ] **Step 4: Commit**

```powershell
git add services/api-gateway/src/main.rs
git commit -m "feat(api-gateway): proxy handler routes via extract+render+forward"
```

---

## Task 6：迁移全部 router.toml

**Files:**
- Modify: `docker/config/api-gateway/routes/audit.router.toml`
- Modify: `docker/config/api-gateway/routes/iam.router.toml`
- Modify: `docker/config/api-gateway/routes/notification.router.toml`
- Modify: `docker/config/api-gateway/routes/permissions.router.toml`
- Modify: `docker/config/api-gateway/routes/prue_backend.router.toml`
- Modify: `docker/config/api-gateway/routes/roles.router.toml`
- Modify: `docker/config/api-gateway/routes/token.router.toml`
- Modify: `docker/config/api-gateway/routes/user.router.toml`

迁移规则（机械替换）：

| 旧 | 新 |
|---|---|
| `match = "/v1/X"` + `strip_prefix /v1` | `upstream = "/v1/X"`，`downstream = "/X"` |
| `match = "/v1/X/**"` + `strip_prefix /v1` | `upstream = "/v1/X/{rest}"`，`downstream = "/X/{rest}"` |
| `match = "/v1/notification/ws"` ws=true | `upstream = "/v1/notification/ws"`，`downstream = "/notification/ws"`, `ws = true` |

- [ ] **Step 1: iam.router.toml**

把每条 `[[route]]` 重写为新写法。示例（仅展示前两条，其他按规则同步）：

```toml
[[route]]
upstream   = "/v1/iam/auth/login"
downstream = "/iam/auth/login"
service    = "iam-service"
public     = true

[[route]]
upstream   = "/v1/iam/auth/register"
downstream = "/iam/auth/register"
service    = "iam-service"
public     = true

[[route]]
upstream   = "/v1/iam/auth/refresh"
downstream = "/iam/auth/refresh"
service    = "iam-service"
public     = true

[[route]]
upstream   = "/v1/iam/auth/setup-status"
downstream = "/iam/auth/setup-status"
service    = "iam-service"
public     = true

[[route]]
upstream   = "/v1/iam/auth/{rest}"
downstream = "/iam/auth/{rest}"
service    = "iam-service"
```

> 把同文件其余条目按相同规则改完。

- [ ] **Step 2: token / audit / notification / user / roles / permissions / prue_backend**

逐文件应用迁移规则：

- 精确路径条目：保留 `upstream = match`，新增 `downstream = upstream 去掉 /v1`
- `/**` 通配：把 upstream 的 `**` 替换成 `{rest}`，downstream 同名变量
- `ws = true` 字段保留
- 删除整段 `rewrite = [...]`
- 注意：旧 notification 文件里 `/v1/notification/health` 排在 `/v1/notification/**` 之后，新 loader 已按字面量长度排序，可保持任意顺序，但请把 `health` 标 `public = true`

- [ ] **Step 3: 启动期 validate**

```powershell
cargo run -p api-gateway -- validate --routes docker/config/api-gateway/routes
```

期望输出：`OK: N routes from ...`，列出所有路由 upstream → service。

- [ ] **Step 4: Commit**

```powershell
git add docker/config/api-gateway/routes/*.router.toml
git commit -m "feat(api-gateway): migrate all routes to upstream/downstream syntax"
```

---

## Task 7：修集成测试与 lib 内部测试

**Files:**
- Modify: `services/api-gateway/tests/integration_test.rs`
- Modify: `services/api-gateway/tests/headers_test.rs`
- Modify: `services/api-gateway/tests/middleware_test.rs`

- [ ] **Step 1: 全文检索旧字段引用**

```powershell
cargo build -p api-gateway --tests
```

定位所有报错点，把 `match_pattern` / `rewrite` / `RewriteSpec` 替换为新字段。常见模式：

```rust
RouteConfig {
    upstream: "/v1/x".into(),
    downstream: "/x".into(),
    service: "s".into(),
    public: false,
    ws: false,
    headers: Default::default(),
}
```

- [ ] **Step 2: 跑全部测试**

```powershell
cargo test -p api-gateway
```

期望：全部 PASS。

- [ ] **Step 3: Commit**

```powershell
git add services/api-gateway/tests
git commit -m "test(api-gateway): align tests with upstream/downstream model"
```

---

## Task 8：同步规则与文档

**Files:**
- Modify: `.trae/rules/routing-rules.md`
- Modify: `.trae/rules/project_rules.md`
- Modify: `docs/guides/api-gateway-routing.md`（如存在）

- [ ] **Step 1: `.trae/rules/routing-rules.md`**

重写 §2.2 路由条目模板：

````markdown
### 2.2 路由条目模板

```toml
[[route]]
upstream   = "/v1/<service>/<resource>/{rest}"
downstream = "/<service>/<resource>/{rest}"
service    = "<service>-service"
public     = false
```

字段说明：

| 字段 | 必填 | 说明 |
|---|---|---|
| `upstream` | 是 | 网关入口模板（已剥除 `/api`） |
| `downstream` | 是 | 转发到微服务的路径模板，可含 query |
| `service` | 是 | Consul 注册名 |
| `public` | 否 | true 跳过 JWT |
| `ws` | 否 | true 升级 WebSocket |
| `headers` | 否 | 转发时附加请求头 |

占位符：`{name}` 提取请求 url 中的字符串到 dict，downstream 做 `{name}` → `dict[name]` 替换。变量值可包含 `/`。
````

删除 §2.3 rewrite 类型白名单，§2.4 改"匹配按字面量长度降序"，§2.5 / §2.6 / §2.7 全部改用 upstream/downstream 例子。

- [ ] **Step 2: `.trae/rules/project_rules.md`**

删除"`rewrite` 类型只能是 `strip_prefix` / `redirect` / ..."一条，替换为：

```markdown
- 路由条目使用 `upstream` / `downstream` 占位符语法，不再支持 `match` / `rewrite`
```

- [ ] **Step 3: `docs/guides/api-gateway-routing.md`**

如已存在，重写"配置语法"章节为占位符模型；如不存在，跳过。

- [ ] **Step 4: Commit**

```powershell
git add .trae/rules docs/guides/api-gateway-routing.md
git commit -m "docs(api-gateway): update routing rules to upstream/downstream syntax"
```

---

## Task 9：端到端验收

**Files:**（无修改）

- [ ] **Step 1: 跑路由校验**

```powershell
cargo run -p api-gateway -- validate --routes docker/config/api-gateway/routes
```

期望：退出码 0。

- [ ] **Step 2: 跑全量单元/集成测试**

```powershell
cargo test -p api-gateway
```

期望：全部 PASS。

- [ ] **Step 3: 跑 e2e（容器）**

```powershell
node scripts/test-orchestrator.js loop
```

期望：现有 e2e（如 `tests/e2e/config.e2e.test.ts` 中 `/v1/config/namespace?page=1&size=10`）保持 200。

- [ ] **Step 4: 输出 summary 并 tag**

```powershell
git tag pre-ocelot-routing-base
git log --oneline -10
```

留作回滚锚点。

---

## Self-Review Notes

- spec §2 / §3 / §4 全部覆盖：模板语法 → Task 1 / Task 4，匹配渲染 → Task 1 / Task 5，校验规则 → Task 4
- spec §5 改动清单：每条都对应一个 Task（rewrite.rs 删除 → Task 3，路由文件迁移 → Task 6，规则同步 → Task 8）
- spec §6 迁移示例已嵌入 Task 6 的迁移规则表
- spec §7 错误处理：404/502/503 路径在 router.rs / main.rs 均保留
- spec §8 测试覆盖：Task 1 单元 + Task 4 加载校验 + Task 7 集成 + Task 9 e2e
- 占位符语义统一：所有 `{name}` 贪婪匹配剩余字符串，无段限定，与 spec §2.2 一致
- 类型一致性：`Template` / `TplPart` / `RouteConfig` 字段名在 Task 1 / 2 / 4 / 5 各处保持同名
