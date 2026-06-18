# Consul Advertise Host 自动发现 设计

**日期**：2026-06-18
**作者**：System Architect
**目标**：让注册到 Consul 的 service `address` 反映容器/主机真实 IP，而非 `127.0.0.1` 或 service name。

---

## 1. 背景

当前 `libs/consul-registrar/src/lib.rs` 把 `host_to_address` 中的 `0.0.0.0` 改写为 `127.0.0.1`：

```rust
fn host_to_address(host: &str, port: u16) -> String {
    if host == "0.0.0.0" {
        format!("127.0.0.1:{}", port)
    } else {
        format!("{}:{}", host, port)
    }
}
```

后果：
- `token-service` 监听 `0.0.0.0:8080`，被改写后注册到 Consul 的 `address = "127.0.0.1"`。Consul agent 从容器外访问 `127.0.0.1:8080` 无法到达该容器，DNS 解析与健康检查失败。
- `iam-service` 已经用 `CONSUL_ADVERTISE_HOST` 兜底，但 host 仍可能不是本机 IP，且环境变量语义含糊。

需求：注册到 Consul 的 `address` 必须是部署主机/容器的真实出向 IP，使 Consul agent 跨容器可访问。

---

## 2. 目标

- 在 `consul-registrar` 库内封装 IP 探测：UDP socket connect 到 consul 地址反查本机 IP（不真正发包）
- 探测失败时 fall back 到调用方传入的 `service_name`，保留 Consul DNS 解析能力
- 移除 `host_to_address` 对 `0.0.0.0` 的静默改写
- `token-service` 与 `iam-service` 统一走新接口

非目标：
- 不保留 `CONSUL_ADVERTISE_HOST` 环境变量
- 不改 docker-compose 网络配置
- 不引入第三方 crate

---

## 3. 文件清单

| 路径 | 变更 | 说明 |
|---|---|---|
| `libs/consul-registrar/src/lib.rs` | 修改 | 新增 `discover_advertise_host` + `register_discovered`；移除 `0.0.0.0` 改写 |
| `services/token-service/src/main.rs` | 修改 | 改用 `register_discovered` |
| `services/iam-service/src/main.rs` | 修改 | 删除 `CONSUL_ADVERTISE_HOST` 读取，改用 `register_discovered` |

---

## 4. 设计

### 4.1 `discover_advertise_host`

签名：

```rust
pub fn discover_advertise_host(consul_url: &str, fallback: &str) -> String
```

行为：
1. 解析 `consul_url`，得到 `host:port`：
   - 剥离 `http://` / `https://` scheme
   - 默认 port：`http` → 8500；`https` → 443
   - 解析失败 → 返回 `fallback`
2. `std::net::UdpSocket::bind("0.0.0.0:0")`：
   - 失败 → `tracing::warn!` + 返回 `fallback`
3. `socket.connect((host.as_str(), port))`：
   - UDP connect 不真正发包，仅设置对端地址
   - 失败 → `tracing::warn!` + 返回 `fallback`
4. `socket.local_addr().ok().map(|a| a.ip().to_string(.)to_string())`：
   - 若为 若0.0.0.0loca P 返 ck`-
   - 其他 →  IP 字符串

### 4.2 `register_discovered`

签名：

```rust
pub async fn register_discovered(
    &mut self,
    port: u16,
    fallback: &str,
) -> Result<(), String>
```

行为：
1. `let host = Self::discover_advertise_host(&self.addr, fallback);`
2. `tracing::info!("advertise host: {} (fallback={})", host, fallback);`
3. 委托给现有 `register(host, port)`，保留 service_id eager persist 与 consul 调用逻辑

### 4.3 `host_to_address` 语义变更

旧行为：把 `0.0.0.0` 改写为 `127.0.0.1`
新行为：原样拼接 `format!("{}:{}", host, port)`

调用方不再传入 `0.0.0.0`（改为传探测结果或 service name），因此该改写不再必要。删除特判逻辑，单测相应更新。

### 4.4 API 兼容

- 保留 `register(host, port)` 公开签名，行为变更为「原样传 host」
- 单测中 `host_to_address_rewrites_unspecified` 改为 `host_to_address_keeps_unspecified`（断言返回 `"0.0.0.0:8080"`）

---

## 5. 调用方变更

### `services/token-service/src/main.rs`

```rust
// 旧
match consul_registrar
    .register(&cfg.server.host, cfg.server.port)
    .await { ... }

// 新
match consul_registrar
    .register_discovered(cfg.server.port, &cfg.consul.service_name)
    .await { ... }
```

### `services/iam-service/src/main.rs`

```rust
// 删除
let advertise_host = std::env::var("CONSUL_ADVERTISE_HOST")
    .unwrap_or_else(|_| config.consul.service_name.clone());

// 替换为
match consul_registrar
    .register_discovered(config.app.port, &config.consul.service_name)
    .await { ... }
```

---

## 6. 单测

| 用例 | 断言 |
|---|---|
| `parse_consul_endpoint_strips_http_scheme` | `"http://consul:8500"` → `("consul", 8500)` |
| `parse_consul_endpoint_strips_https_scheme` | `"https://consul"` → `("consul", 443)` |
| `parse_consul_endpoint_returns_none_on_garbage` | `"not a url"` → `None` |
| `parse_consul_endpoint_returns_none_on_missing_host` | `"http://"` → `None` |
| `discover_advertise_host_returns_fallback_on_invalid_url` | 非法 URL → fallback（`parse_consul_endpoint` 失败） |
| `discover_advertise_host_returns_fallback_on_bind_failure` | 注入不可 bind 的环境 → fallback |
| `discover_advertise_host_returns_local_ip_when_connect_succeeds` | bind 0.0.0.0 + connect 到本地可达地址 → 返回非空 IP |
| `discover_advertise_host_returns_fallback_on_unspecified_local` | mock `probe_local_ip` 返回 `0.0.0.0` → fallback |
| `register_discovered_persists_id_when_consul_unreachable` | 复用现有 eager persist 行为 |
| `host_to_address_keeps_unspecified` | `host_to_address("0.0.0.0", 8080) == "0.0.0.0:8080"` |

mock 策略：单测不真起 UDP 探测（依赖运行时网络）。拆出两个纯函数：
- `parse_consul_endpoint(url: &str) -> Option<(String, u16)>`：纯字符串解析，单测覆盖各种 URL 形态
- `probe_local_ip(target: SocketAddr) -> Option<IpAddr>`：UDP bind + connect，标记 `#[cfg(test)] pub(crate)` 或经由 trait 注入；测试时连接到 `127.0.0.1:<unused>` 反查本机回环 IP

`discover_advertise_host` 调用两者并组合 fallback 逻辑，不直接做 IO。

---

## 7. 错误处理

| 场景 | 行为 |
|---|---|
| `consul_url` 解析失败 | warn + fallback |
| UDP bind 失败 | warn + fallback |
| UDP connect 失败 | warn + fallback |
| 本地 IP 为 unspecified / link-local | warn + fallback |
| Consul HTTP 调用失败 | 现有 warn 行为，不 panic |
| service_id 文件写入失败 | 现有 warn 行为，service_id 旋转 |

启动不阻塞，consul 注册失败仅 warn。

---

## 8. 风险与权衡

- **运行时探测依赖本地网络栈**：在受限环境（如某些 K8s NetworkPolicy）下可能失败 → fallback 到 service name 仍能工作（依赖 Docker DNS）
- **UDP connect 不是真正握手**：纯本地操作，无对外流量，无副作用
- **不保留 `CONSUL_ADVERTISE_HOST`**：简化心智模型；特殊场景（固定 IP 部署）若需要可改用 service_name 解析或后续重新引入
- **改 `host_to_address` 行为是破坏性变更**：仅影响 `consul-registrar` 的两个调用方（token-service、iam-service），本次同步更新

---

## 9. 不做的事

- 不引入 `getifaddrs` / `if-addrs` crate
- 不支持 IPv6 link-local 优先
- 不缓存探测结果（启动期一次性调用，无收益）
- 不改 docker-compose.yml
- 不改 AGENTS.md / 项目规则