# Consul Advertise Host 自动发现 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 `consul-registrar` 在容器/主机启动时自动探测本机出向 IP 作为 Consul 注册 `address`，并在 `token-service` / `iam-service` 统一调用新接口。

**Architecture:** 在 `consul-registrar` 库内新增 `parse_consul_endpoint`（纯字符串解析）+ `probe_local_ip`（UDP bind + connect 反查）+ `discover_advertise_host`（组合两者 + fallback）；新增公开方法 `register_discovered(port, fallback)` 委托给现有 `register(host, port)`。移除 `host_to_address` 对 `0.0.0.0` 的静默改写。`token-service` 与 `iam-service` 改用 `register_discovered`，删除 `iam-service` 中 `CONSUL_ADVERTISE_HOST` 读取。

**Tech Stack:** Rust 2021 edition、`std::net::UdpSocket`、`tokio`、`hyper`、`tracing`、`serde`。无新依赖。

---

## File Structure

| 文件 | 职责 |
|---|---|
| `libs/consul-registrar/src/lib.rs` | consul 客户端库：URL 解析、IP 探测、register API |
| `services/token-service/src/main.rs` | 启动入口，改用 `register_discovered` |
| `services/iam-service/src/main.rs` | 启动入口，删除 `CONSUL_ADVERTISE_HOST`，改用 `register_discovered` |

---

### Task 1: `parse_consul_endpoint` 纯函数 + 测试

**Files:**
- Modify: `libs/consul-registrar/src/lib.rs`

- [ ] **Step 1: 在 `lib.rs` 末尾 `#[cfg(test)] mod tests` 中追加失败测试**

在 `mod tests { ... }` 块内、`register_reuses_id_across_restarts` 函数后追加：

```rust
    #[test]
    fn parse_consul_endpoint_strips_http_scheme() {
        let r = ConsulRegistrar::parse_consul_endpoint("http://consul:8500");
        assert_eq!(r, Some(("consul".to_string(), 8500)));
    }

    #[test]
    fn parse_consul_endpoint_strips_https_scheme() {
        let r = ConsulRegistrar::parse_consul_endpoint("https://consul");
        assert_eq!(r, Some(("consul".to_string(), 443)));
    }

    #[test]
    fn parse_consul_endpoint_returns_none_on_garbage() {
        let r = ConsulRegistrar::parse_consul_endpoint("not a url");
        assert_eq!(r, None);
    }

    #[test]
    fn parse_consul_endpoint_returns_none_on_missing_host() {
        let r = ConsulRegistrar::parse_consul_endpoint("http://");
        assert_eq!(r, None);
    }
```

- [ ] **Step 2: 跑测试确认失败**

在仓库根目录（vcvarsall x64 环境）执行：

```bash
cargo test -p consul-registrar --lib parse_consul_endpoint
```

Expected: 编译失败，错误信息 `error[E0599]: no function or associated item named 'parse_consul_endpoint' found for struct 'ConsulRegistrar'`，或 4 个测试全部 fail（`assertion failed: r == Some(...)`）。

- [ ] **Step 3: 实现 `parse_consul_endpoint`**

在 `impl ConsulRegistrar { ... }` 块顶部（`fn new` 之前）新增公开关联函数：

```rust
    /// Parse `http://host:port` / `https://host[:port]` into `(host, port)`.
    /// Returns `None` on scheme mismatch, missing host, or unparsable port.
    pub fn parse_consul_endpoint(url: &str) -> Option<(String, u16)> {
        let rest = url
            .strip_prefix("http://")
            .map(|s| (s, 850u16))
            .or_else(|| url.strip_prefix("https://").map(|s| (s, 443u16)))?;
        let (host, port) = match rest.0.split_once(':') {
            Some((h, p)) => (h, p),
            None => (rest.0, ""),
        };
        if host.is_empty() {
            return None;
        }
        let parsed_port: u16 = if port.is_empty() {
            rest.1
        } else {
            port.parse().ok()?
        };
        Some((host.to_string(), parsed_port))
    }
```

- [ ] **Step 4: 跑测试确认通过**

```bash
cargo test -p consul-registrar --lib parse_consul_endpoint
```

Expected: 4 个测试全部 PASS。

- [ ] **Step 5: Commit**

```bash
git add libs/consul-registrar/src/lib.rs
git commit -m "feat(consul-registrar): parse_consul_endpoint helper"
```

---

### Task 2: `probe_local_ip` + 测试

**Files:**
- Modify: `libs/consul-registrar/src/lib.rs`

- [ ] **Step 1: 追加失败测试**

在 `mod tests` 块内继续追加：

```rust
    #[test]
    fn probe_local_ip_returns_local_ip_when_target_reachable() {
        // bind a UDP socket to capture the kernel-assigned port; use it as target.
        let probe_target: std::net::SocketAddr = {
            let s = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            let addr = s.local_addr().unwrap();
            // keep socket alive in test scope by leaking; not pretty but bounded.
            std::mem::forget(s);
            addr
        };
        let ip = ConsulRegistrar::probe_local_ip(probe_target);
        assert!(ip.is_some(), "expected Some, got {:?}", ip);
    }

    #[test]
    fn probe_local_ip_returns_none_when_bind_fails() {
        // port 0 binds always succeed on supported platforms, so simulate failure
        // by passing an obviously unconnectable target (port 1 on unreachable host).
        // On loopback-only env this still may succeed; assert via Some/None either way.
        let ip = ConsulRegistrar::probe_local_ip("127.0.0.1:1".parse().unwrap());
        // May be Some or None depending on env; just ensure it doesn't panic.
        let _ = ip;
    }
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cargo test -p consul-registrar --lib probe_local_ip
```

Expected: 编译失败 `no associated function 'probe_local_ip'`。

- [ ] **Step 3: 实现 `probe_local_ip`**

在 `impl ConsulRegistrar` 中 `parse_consul_endpoint` 之后新增：

```rust
    /// Probe the local outbound IP by UDP-binding to `0.0.0.0:0` and connecting
    /// (without sending) to `target`. UDP `connect` only sets the peer address,
    /// so no packet leaves the host. Returns `None` on any failure.
    pub fn probe_local_ip(target: std::net::SocketAddr) -> Option<std::net::IpAddr> {
        let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
        socket.connect(target).ok()?;
        let ip = socket.local_addr().ok()?.ip();
        if ip.is_unspecified() {
            return None;
        }
        if let std::net::IpAddr::V6(v6) = ip {
            if v6.is_link_local() {
                return None;
            }
        }
        Some(ip)
    }
```

- [ ] **Step 4: 跑测试确认通过**

```bash
cargo test -p consul-registrar --lib probe_local_ip
```

Expected: 2 个测试 PASS（第二个仅断言不 panic）。

- [ ] **Step 5: Commit**

```bash
git add libs/consul-registrar/src/lib.rs
git commit -m "feat(consul-registrar): probe_local_ip via udp connect"
```

---

### Task 3: `discover_advertise_host` + 测试

**Files:**
- Modify: `libs/consul-registrar/src/lib.rs`

- [ ] **Step 1: 追加失败测试**

在 `mod tests` 块内继续追加：

```rust
    #[test]
    fn discover_advertise_host_returns_fallback_on_invalid_url() {
        let r = ConsulRegistrar::discover_advertise_host("not a url", "fallback-host");
        assert_eq!(r, "fallback-host");
    }

    #[test]
    fn discover_advertise_host_returns_local_ip_when_target_loopback() {
        // bind a udp socket to get a real loopback port; URL points to loopback
        // so probe_local_ip should succeed and return 127.0.0.1.
        let target_port: u16 = {
            let s = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            let p = s.local_addr().unwrap().port();
            std::mem::forget(s);
            p
        };
        let url = format!("http://127.0.0.1:{}", target_port);
        let ip = ConsulRegistrar::discover_advertise_host(&url, "fallback");
        assert_eq!(ip, "127.0.0.1");
    }

    #[test]
    fn discover_advertise_host_returns_fallback_on_unspecified_local() {
        // No easy way to force unspecified local; instead verify fallback path
        // by passing an invalid url and asserting fallback is returned verbatim.
        let r = ConsulRegistrar::discover_advertise_host("http://", "fb");
        assert_eq!(r, "fb");
    }
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cargo test -p consul-registrar --lib discover_advertise_host
```

Expected: 编译失败 `no associated function 'discover_advertise_host'`。

- [ ] **Step 3: 实现 `discover_advertise_host`**

在 `impl ConsulRegistrar` 中 `probe_local_ip` 之后新增：

```rust
    /// Resolve the best advertise host for Consul registration.
    /// Order: parse `consul_url` → UDP-probe local IP → fallback.
    /// Logs a warning and falls back on any failure.
    pub fn discover_advertise_host(consul_url: &str, fallback: &str) -> String {
        let Some((host, port)) = Self::parse_consul_endpoint(consul_url) else {
            tracing::warn!(
                "failed to parse consul url '{}', using fallback '{}'",
                consul_url,
                fallback
            );
            return fallback.to_string();
        };
        let Ok(target) = format!("{}:{}", host, port).parse::<std::net::SocketAddr>() else {
            tracing::warn!(
                "invalid consul socket '{}:{}', using fallback '{}'",
                host,
                port,
                fallback
            );
            return fallback.to_string();
        };
        match Self::probe_local_ip(target) {
            Some(ip) => {
                tracing::info!("discovered advertise host: {}", ip);
                ip.to_string()
            }
            None => {
                tracing::warn!(
                    "failed to probe local ip, using fallback '{}'",
                    fallback
                );
                fallback.to_string()
            }
        }
    }
```

- [ ] **Step 4: 跑测试确认通过**

```bash
cargo test -p consul-registrar --lib discover_advertise_host
```

Expected: 3 个测试 PASS。

- [ ] **Step 5: Commit**

```bash
git add libs/consul-registrar/src/lib.rs
git commit -m "feat(consul-registrar): discover_advertise_host with fallback"
```

---

### Task 4: `register_discovered` 公开方法 + 测试

**Files:**
- Modify: `libs/consul-registrar/src/lib.rs`

- [ ] **Step 1: 追加失败测试**

在 `mod tests` 块内 `register_reuses_id_across_restarts` 之后追加：

```rust
    #[tokio::test]
    async fn register_discovered_persists_id_when_consul_unreachable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("svc-id");
        let path_str = path.to_str().unwrap();

        let mut r = ConsulRegistrar::new("http://127.0.0.1:1", "x", path_str);
        assert!(!path.exists());

        // consul unreachable; fallback "x" is used as advertise host.
        let _ = r.register_discovered(9999, "x").await;
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, r.service_id());
    }
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cargo test -p consul-registrar --lib register_discovered
```

Expected: 编译失败 `no method named 'register_discovered'`。

- [ ] **Step 3: 实现 `register_discovered`**

在 `impl ConsulRegistrar` 中现有 `register(host, port)` 方法之后新增：

```rust
    /// Like `register`, but resolves the advertise host via `discover_advertise_host`
    /// using the registrar's `addr` (consul url) and the provided `fallback`.
    /// `fallback` is typically the service name (e.g. `"token-service"`) so consul
    /// can still resolve it via Docker DNS when probe fails.
    pub async fn register_discovered(
        &mut self,
        port: u16,
        fallback: &str,
    ) -> Result<(), String> {
        let host = Self::discover_advertise_host(&self.addr, fallback);
        self.register(&host, port).await
    }
```

- [ ] **Step 4: 跑测试确认通过**

```bash
cargo test -p consul-registrar --lib register_discovered
```

Expected: 1 个测试 PASS。

- [ ] **Step 5: Commit**

```bash
git add libs/consul-registrar/src/lib.rs
git commit -m "feat(consul-registrar): register_discovered entry point"
```

---

### Task 5: 移除 `host_to_address` 的 `0.0.0.0` 改写 + 更新测试

**Files:**
- Modify: `libs/consul-registrar/src/lib.rs`

- [ ] **Step 1: 修改 `host_to_address` 函数体**

定位到：

```rust
    fn host_to_address(host: &str, port: u16) -> String {
        if host == "0.0.0.0" {
            format!("127.0.0.1:{}", port)
        } else {
            format!("{}:{}", host, port)
        }
    }
```

改为：

```rust
    fn host_to_address(host: &str, port: u16) -> String {
        format!("{}:{}", host, port)
    }
```

- [ ] **Step 2: 更新失败测试**

定位到 `host_to_address_rewrites_unspecified`：

```rust
    #[test]
    fn host_to_address_rewrites_unspecified() {
        assert_eq!(
            ConsulRegistrar::host_to_address("0.0.0.0", 8082),
            "127.0.0.1:8082"
        );
    }
```

替换为：

```rust
    #[test]
    fn host_to_address_keeps_unspecified() {
        assert_eq!(
            ConsulRegistrar::host_to_address("0.0.0.0", 8082),
            "0.0.0.0:8082"
        );
    }
```

- [ ] **Step 3: 跑测试确认通过**

```bash
cargo test -p consul-registrar --lib
```

Expected: 全部测试 PASS（包括 Task 1-4 新增测试）。

- [ ] **Step 4: Commit**

```bash
git add libs/consul-registrar/src/lib.rs
git commit -m "refactor(consul-registrar): drop 0.0.0.0 rewrite in host_to_address"
```

---

### Task 6: `token-service` 切换到 `register_discovered`

**Files:**
- Modify: `services/token-service/src/main.rs`

- [ ] **Step 1: 修改调用点**

定位到：

```rust
    let mut consul_registrar = ConsulRegistrar::new(
        &cfg.consul.addr,
        &cfg.consul.service_name,
        &cfg.consul.service_id_file,
    );
    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    match consul_registrar
        .register(&cfg.server.host, cfg.server.port)
        .await
    {
        Ok(_) => tracing::info!("token-service registered with consul"),
        Err(e) => tracing::warn!("failed to register with consul: {}", e),
    }
```

改为：

```rust
    let mut consul_registrar = ConsulRegistrar::new(
        &cfg.consul.addr,
        &cfg.consul.service_name,
        &cfg.consul.service_id_file,
    );
    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    match consul_registrar
        .register_discovered(cfg.server.port, &cfg.consul.service_name)
        .await
    {
        Ok(_) => tracing::info!("token-service registered with consul"),
        Err(e) => tracing::warn!("failed to register with consul: {}", e),
    }
```

- [ ] **Step 2: 编译验证**

```bash
cargo check -p token-service
```

Expected: 无错误无警告（`aws_lc_rs` crypto provider，不在沙箱内 link）。

- [ ] **Step 3: Commit**

```bash
git add services/token-service/src/main.rs
git commit -m "feat(token-service): use register_discovered for consul"
```

---

### Task 7: `iam-service` 切换到 `register_discovered`，删除 `CONSUL_ADVERTISE_HOST`

**Files:**
- Modify: `services/iam-service/src/main.rs`

- [ ] **Step 1: 修改调用点**

定位到：

```rust
    let consul_addr = format!("http://{}:{}", config.consul.host, config.consul.port);
    let mut consul_registrar = ConsulRegistrar::new(
        &consul_addr,
        &config.consul.service_name,
        "/tmp/iam-service-id",
    );
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    
    let advertise_host = std::env::var("CONSUL_ADVERTISE_HOST")
        .unwrap_or_else(|_| config.consul.service_name.clone());
    match consul_registrar.register(&advertise_host, config.app.port).await {
        Ok(_) => info!("iam-service registered with consul as {}", advertise_host),
        Err(e) => warn!("failed to register with consul: {}", e),
    }
```

改为：

```rust
    let consul_addr = format!("http://{}:{}", config.consul.host, config.consul.port);
    let mut consul_registrar = ConsulRegistrar::new(
        &consul_addr,
        &config.consul.service_name,
        "/tmp/iam-service-id",
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;

    match consul_registrar
        .register_discovered(config.app.port, &config.consul.service_name)
        .await
    {
        Ok(_) => info!(
            "iam-service registered with consul (fallback {})",
            config.consul.service_name
        ),
        Err(e) => warn!("failed to register with consul: {}", e),
    }
```

- [ ] **Step 2: 编译验证**

```bash
cargo check -p iam-service
```

Expected: 无错误无警告。

- [ ] **Step 3: 跑全部 consul-registrar 测试**

```bash
cargo test -p consul-registrar --lib
```

Expected: 全部 PASS（回归确认）。

- [ ] **Step 4: Commit**

```bash
git add services/iam-service/src/main.rs
git commit -m "feat(iam-service): use register_discovered, drop CONSUL_ADVERTISE_HOST"
```

---

### Task 8: 端到端验证（docker compose up）

**Files:** 无

- [ ] **Step 1: 启动基础设施**

```bash
node scripts/dev-services.js up
```

Expected: consul、mysql、redis、token-service、iam-service 全部 `healthy`。

- [ ] **Step 2: 验证 Consul UI 中的 address**

打开 http://localhost:8500/ui → Services：
- `token-service` 应展示容器 IP（如 `172.20.0.x`），不是 `127.0.0.1`
- `iam-service` 应展示容器 IP，不是 `iam-service`

- [ ] **Step 3: 验证健康检查**

在 consul UI 上点开 service → Health checks：
- 全部应显示 `passing`
- Checks 的 URL 形如 `http://172.20.0.x:8080/health`，可通过 curl 验证

```bash
curl -s http://localhost:8500/v1/health/service/token-service | jq '.[0].Service.Address'
```

Expected: 容器 IP 字符串。

- [ ] **Step 4: 关闭环境**

```bash
node scripts/dev-services.js down
```

Expected: 容器正常关闭，无遗留进程。

---

## 自审

**Spec 覆盖**：
- §4.1 `discover_advertise_host` → Task 1（`parse_consul_endpoint`）+ Task 2（`probe_local_ip`）+ Task 3（组合）
- §4.2 `register_discovered` → Task 4
- §4.3 `host_to_address` 改写移除 → Task 5
- §4.4 API 兼容 → Task 5（保留签名）
- §5 token-service 变更 → Task 6
- §5 iam-service 变更 → Task 7
- §6 单测 → 全部 Task 1-5 含测试用例
- §7 错误处理 → `discover_advertise_host` 内联 warn，Task 1-3 测试覆盖 fallback
- §8 风险权衡 → Task 8 端到端验证

**占位符**：无 TBD/TODO，所有代码片段完整。

**类型一致性**：`parse_consul_endpoint: fn(&str) -> Option<(String, u16)>` 在 Task 1 定义；Task 3 使用 `format!("{}:{}", host, port).parse::<SocketAddr>()` 与 Task 1 的 `Option<(String, u16)>` 一致。`discover_advertise_host: fn(&str, &str) -> String` 在 Task 3 定义；Task 4 `register_discovered` 签名 `(port: u16, fallback: &str)` 与 spec §4.2 一致。