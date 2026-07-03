# Phase 5-A：Consul Connect 服务网格 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 docker-compose 上引入 Consul Connect 服务网格，实现服务间 mTLS + 意图授权 + mesh 可观测性，淘汰应用层 service-token。

**Architecture:** 每个服务加一个 Consul Connect Envoy sidecar 容器（`consul:1.15`）。app 绑定 `127.0.0.1:18080`（仅容器内），sidecar 绑定网络端口（mTLS 终止 + 转发）。调用方通过本地 sidecar upstream 端口访问上游服务。Consul 内置 CA 自动签发/轮换证书。意图（intentions）替代 `require_service_jwt` 的服务身份校验。

**Tech Stack:** Consul 1.15 Connect · Envoy sidecar · Rust（consul-registrar 库扩展）· docker-compose

## Global Constraints

- 项目处于开发阶段，从未发布，无遗留数据，不考虑向后兼容——一次性全量切换
- 统一基础镜像 `consul:1.15`（已存在 docker-compose-infra.yml）
- Rust 编译用 `zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl`，target `x86_64-unknown-linux-musl`
- TLS 依赖统一 workspace `reqwest 0.12 + rustls`，禁用 native-tls
- 从 GitHub 获取文件必须用 `gh-proxy.org` 代理
- AI 回复与思考过程必须使用中文

---

## 出站 Upstream 端口分配表

| 调用方 | 目标服务 | 本地监听端口 |
|--------|----------|-------------|
| api-gateway | iam-service | 127.0.0.1:9081 |
| api-gateway | token-service | 127.0.0.1:9082 |
| api-gateway | notification-service | 127.0.0.1:9083 |
| api-gateway | audit-service | 127.0.0.1:9084 |
| api-gateway | config-service | 127.0.0.1:9085 |
| api-gateway | file-service | 127.0.0.1:9086 |
| api-gateway | prue_backend | 127.0.0.1:9087 |
| token-service | iam-service | 127.0.0.1:9081 |
| notification-service | iam-service | 127.0.0.1:9081 |

---

## File Structure

| 文件 | 职责 | 操作 |
|------|------|------|
| `libs/consul-registrar/src/lib.rs` | 加 ConnectConfig/Upstream 结构 + with_upstreams builder | Modify |
| `infrastructure/docker-compose-infra.yml` | Consul 启用 Connect | Modify |
| `infrastructure/config/consul/connect.hcl` | 已存在 `connect { enabled = true }` | 不变 |
| `scripts/consul-intentions.sh` | 意图矩阵声明式管理 | Create |
| `infrastructure/docker-compose-services.yml` | 每服务加 sidecar 容器 + 绑定地址 | Modify |
| `infrastructure/config/api-gateway/api-gateway.toml` | egress_map 改本地 sidecar 端口 | Modify |
| `services/token-service/src/config.rs` | user_service_url 默认值改本地 sidecar | Modify |
| `services/notification-service/src/config.rs` | iam_base_url 默认值改本地 sidecar | Modify |
| `services/*/src/main.rs` | ConsulRegistrar 加 connect + upstreams | Modify |
| `services/iam-service/src/middleware/service_jwt.rs` | 删除 | Delete |
| `services/iam-service/src/middleware/mod.rs` | 删 `pub mod service_jwt` | Modify |
| `services/iam-service/src/routes/internal_users.rs` | 移除 require_service_jwt 挂载 | Modify |
| `services/iam-service/tests/internal_users_test.rs` | 改测试 fixture（不再 mint aud=service token） | Modify |
| `services/notification-service/src/iam_client.rs` | IamMemberClient 保留 token（仍需传 tid JWT） | 不变 |
| `test-case/51-mtls-mesh.md` | 手工验收文档 | Create |

---

### Task 1: ConsulRegistrar 扩展 Connect sidecar + upstreams

**Files:**
- Modify: `libs/consul-registrar/src/lib.rs`
- Test: `libs/consul-registrar/src/lib.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Produces: `pub struct Upstream { destination_name, local_bind_port }` · `ConsulRegistrar::with_upstreams(self, Vec<Upstream>) -> Self`
- 注册 payload 序列化后含 `"Connect": { "SidecarService": { "Proxy": { "Upstreams": [...] } } }`

- [ ] **Step 1: 写失败测试 — connect 字段序列化（无 upstream）**

在 `libs/consul-registrar/src/lib.rs` 的 `mod tests` 末尾加：

```rust
    #[test]
    fn registration_serializes_connect_sidecar_without_upstreams() {
        let reg = AgentServiceRegistration {
            id: "iam-service-uuid".into(),
            name: "iam-service".into(),
            address: "127.0.0.1".into(),
            port: 18080,
            check: AgentServiceCheck {
                http: "http://127.0.0.1:18080/health".into(),
                interval: "10s".into(),
                timeout: "5s".into(),
                deregister_critical_service_after: "1m".into(),
            },
            connect: ConnectConfig {
                sidecar_service: SidecarConfig { proxy: None },
            },
        };
        let json = serde_json::to_value(&reg).unwrap();
        assert_eq!(json["Connect"]["SidecarService"], serde_json::json!({}));
    }

    #[test]
    fn registration_serializes_connect_sidecar_with_upstreams() {
        let reg = AgentServiceRegistration {
            id: "api-gateway-uuid".into(),
            name: "api-gateway".into(),
            address: "127.0.0.1".into(),
            port: 18080,
            check: AgentServiceCheck {
                http: "http://127.0.0.1:18080/health".into(),
                interval: "10s".into(),
                timeout: "5s".into(),
                deregister_critical_service_after: "1m".into(),
            },
            connect: ConnectConfig {
                sidecar_service: SidecarConfig {
                    proxy: Some(ProxyConfig {
                        upstreams: vec![Upstream {
                            destination_name: "iam-service".into(),
                            local_bind_port: 9081,
                        }],
                    }),
                },
            },
        };
        let json = serde_json::to_value(&reg).unwrap();
        assert_eq!(json["Connect"]["SidecarService"]["Proxy"]["Upstreams"][0]["DestinationName"], "iam-service");
        assert_eq!(json["Connect"]["SidecarService"]["Proxy"]["Upstreams"][0]["LocalBindPort"], 9081);
    }

    #[test]
    fn with_upstreams_stores_upstreams_for_registration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("svc-id");
        let r = ConsulRegistrar::new("http://consul:8500", "api-gateway", path.to_str().unwrap())
            .with_upstreams(vec![Upstream {
                destination_name: "iam-service".into(),
                local_bind_port: 9081,
            }]);
        assert_eq!(r.upstreams.len(), 1);
        assert_eq!(r.upstreams[0].destination_name, "iam-service");
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `node scripts/run-tests-docker.js consul-registrar`（或在容器内 `cargo test -p consul-registrar`）
Expected: 编译失败 — `ConnectConfig` / `SidecarConfig` / `ProxyConfig` / `Upstream` 未定义；`AgentServiceRegistration` 无 `connect` 字段；`with_upstreams` 方法不存在

- [ ] **Step 3: 实现扩展**

在 `libs/consul-registrar/src/lib.rs` 中做以下修改：

3a. 在 `AgentServiceCheck` struct 定义后，加新结构：

```rust
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct ConnectConfig {
    sidecar_service: SidecarConfig,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "PascalCase")]
struct SidecarConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    proxy: Option<ProxyConfig>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct ProxyConfig {
    upstreams: Vec<Upstream>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct Upstream {
    destination_name: String,
    local_bind_port: u16,
}

impl Upstream {
    pub fn new(destination_name: &str, local_bind_port: u16) -> Self {
        Self {
            destination_name: destination_name.to_string(),
            local_bind_port,
        }
    }
}
```

3b. 给 `AgentServiceRegistration` 加 `connect` 字段：

```rust
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct AgentServiceRegistration {
    id: String,
    name: String,
    address: String,
    port: u16,
    check: AgentServiceCheck,
    connect: ConnectConfig,
}
```

3c. 给 `ConsulRegistrar` 加 `upstreams` 字段 + builder 方法：

在 `ConsulRegistrar` struct 定义中加：

```rust
pub struct ConsulRegistrar {
    http_client: HttpClient,
    addr: String,
    service_name: String,
    service_id: String,
    service_id_file: String,
    persisted: bool,
    upstreams: Vec<Upstream>,
}
```

在 `new()` 方法中初始化 `upstreams: Vec::new()`：

```rust
    pub fn new(addr: &str, service_name: &str, service_id_file: &str) -> Self {
        let http_client: HttpClient =
            Client::builder(TokioExecutor::new()).build(HttpConnector::new());
        let (service_id, persisted) = Self::load_service_id(service_name, service_id_file);

        Self {
            http_client,
            addr: addr.to_string(),
            service_name: service_name.to_string(),
            service_id,
            service_id_file: service_id_file.to_string(),
            persisted,
            upstreams: Vec::new(),
        }
    }
```

加 builder 方法（在 `new()` 后）：

```rust
    /// Configure upstream services for Connect sidecar proxy.
    /// Each upstream creates a local listener that does mTLS to the target service.
    pub fn with_upstreams(mut self, upstreams: Vec<Upstream>) -> Self {
        self.upstreams = upstreams;
        self
    }
```

3d. 在 `register()` 方法中构造 `connect` 字段。将 `AgentServiceRegistration` 构造改为：

```rust
        let registration = AgentServiceRegistration {
            id: self.service_id.clone(),
            name: self.service_name.clone(),
            address: address.clone(),
            port,
            check: AgentServiceCheck {
                http: format!("http://{}:{}/health", address, port),
                interval: "10s".to_string(),
                timeout: "5s".to_string(),
                deregister_critical_service_after: "1m".to_string(),
            },
            connect: ConnectConfig {
                sidecar_service: SidecarConfig {
                    proxy: if self.upstreams.is_empty() {
                        None
                    } else {
                        Some(ProxyConfig {
                            upstreams: self.upstreams.clone(),
                        })
                    },
                },
            },
        };
```

3e. 修复现有测试 `registration_serializes_plain_service`（它现在需要 `connect` 字段）：

```rust
    #[test]
    fn registration_serializes_plain_service() {
        let reg = AgentServiceRegistration {
            id: "audit-service-abc-uuid".into(),
            name: "audit-service".into(),
            address: "127.0.0.1".into(),
            port: 8080,
            check: AgentServiceCheck {
                http: "http://127.0.0.1:8080/health".into(),
                interval: "10s".into(),
                timeout: "5s".into(),
                deregister_critical_service_after: "1m".into(),
            },
            connect: ConnectConfig {
                sidecar_service: SidecarConfig { proxy: None },
            },
        };
        let json = serde_json::to_value(&reg).unwrap();
        assert_eq!(json["Check"]["HTTP"], "http://127.0.0.1:8080/health");
        assert_eq!(json["Check"]["DeregisterCriticalServiceAfter"], "1m");
        assert_eq!(json["Id"], "audit-service-abc-uuid");
        assert_eq!(json["Name"], "audit-service");
        assert_eq!(json["Connect"]["SidecarService"], serde_json::json!({}));
    }
```

- [ ] **Step 4: 运行测试确认通过**

Run: `node scripts/run-tests-docker.js consul-registrar`
Expected: PASS — 所有测试通过（原有 + 3 个新测试）

- [ ] **Step 5: Commit**

```bash
git add libs/consul-registrar/src/lib.rs
git commit -m "feat(consul-registrar): add Connect sidecar + upstreams support for Consul Connect mesh"
```

---

### Task 2: Consul Connect 基础设施启用 + 意图脚本

**Files:**
- Modify: `infrastructure/docker-compose-infra.yml`
- Create: `scripts/consul-intentions.sh`

**Interfaces:**
- Consul 启动时挂载 `connect.hcl`，启用 Connect
- 意图脚本幂等写入全量 allow/deny 矩阵

- [ ] **Step 1: 修改 docker-compose-infra.yml 启用 Connect**

将 `infrastructure/docker-compose-infra.yml` 的 consul 服务改为：

```yaml
  consul:
    image: consul:1.15
    ports:
      - "8500:8500"
      - "8502:8502"
    volumes:
      - ./config/consul/connect.hcl:/consul/config/connect.hcl:ro
    command: agent -dev -node=consul-server -bind=0.0.0.0 -client=0.0.0.0
    healthcheck:
      test: ["CMD-SHELL", "consul members && nc -z 127.0.0.1 8502"]
      interval: 10s
      timeout: 5s
      retries: 5
    networks:
      - superpowers
```

改动点：取消注释 `volumes` 里的 `connect.hcl` 挂载行（删除 `services.json` 那行注释）。

- [ ] **Step 2: 创建意图脚本**

创建 `scripts/consul-intentions.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail

# Phase 5-A: Consul Connect intention matrix.
# Idempotent: uses `consul intention create -replace` to overwrite.
# Run after `docker-compose up` (Consul must be reachable at localhost:8500).

CONSUL_ADDR="${CONSUL_ADDR:-http://localhost:8500}"
export CONSUL_HTTP_ADDR="$CONSUL_ADDR"

echo "Applying Consul Connect intentions..."

# Allow: api-gateway → all downstream services
consul intention create -replace -allow api-gateway iam-service
consul intention create -replace -allow api-gateway token-service
consul intention create -replace -allow api-gateway notification-service
consul intention create -replace -allow api-gateway audit-service
consul intention create -replace -allow api-gateway config-service
consul intention create -replace -allow api-gateway file-service
consul intention create -replace -allow api-gateway prue_backend

# Allow: token-service → iam-service (fetch_json /internal/users/:id/claims)
consul intention create -replace -allow token-service iam-service

# Allow: notification-service → iam-service (/iam/internal/users/by-{role,dept})
consul intention create -replace -allow notification-service iam-service

# Default deny: everything else
consul intention create -replace -deny '*' '*'

echo "Intentions applied successfully."
consul intention list
```

- [ ] **Step 3: 赋予执行权限**

Run: `chmod +x scripts/consul-intentions.sh`
Expected: 无输出（成功）

- [ ] **Step 4: 验证 Consul Connect 启用**

Run: `cd infrastructure && docker-compose -f docker-compose-infra.yml up -d consul && sleep 5 && docker exec infrastructure-consul-1 consul connect ca config`
Expected: 输出 CA 配置 JSON（含 `ProviderType`: `consul`），说明 Connect 已启用且内置 CA 就绪。

清理: `docker-compose -f docker-compose-infra.yml down`

- [ ] **Step 5: Commit**

```bash
git add infrastructure/docker-compose-infra.yml scripts/consul-intentions.sh
git commit -m "infra(consul): enable Connect + intention matrix script"
```

---

### Task 3: 服务 sidecar 配置 + 绑定地址 + 出站地址

> 这是最核心的改动。每个服务加 sidecar 容器，app 绑定 127.0.0.1:18080，调用方出站地址改本地 sidecar upstream 端口。所有改动必须一起做才能跑通。

**Files:**
- Modify: `infrastructure/docker-compose-services.yml`
- Modify: `infrastructure/config/api-gateway/api-gateway.toml`
- Modify: `services/token-service/src/config.rs`
- Modify: `services/notification-service/src/config.rs`
- Modify: `services/api-gateway/src/main.rs`
- Modify: `services/token-service/src/main.rs`
- Modify: `services/notification-service/src/main.rs`
- Modify: `services/iam-service/src/main.rs`
- Modify: `services/audit-service/src/main.rs`
- Modify: `services/config-service/src/main.rs`
- Modify: `services/file-service/src/main.rs`
- Modify: `services/prue_backend/src/main.rs`

**Interfaces:**
- Consumes: `ConsulRegistrar::with_upstreams` + `Upstream::new` (Task 1)
- Produces: 全栈 mesh 网络 — app 仅 localhost 可达，sidecar 做 mTLS

**Sidecar 容器模板**（每个服务相同模式，替换 `<svc>` / `<port>`）：

```yaml
  <svc>-sidecar:
    image: consul:1.15
    ports:
      - "<host-port>:<svc-port>"
    command: >
      sh -c "until consul catalog services 2>/dev/null | grep -q <svc>; do echo 'waiting for <svc> registration...'; sleep 1; done
      && consul connect envoy -sidecar-for <svc> -grpc-addr=consul:8502 -admin-bind=0.0.0.0:19000"
    depends_on:
      consul:
        condition: service_healthy
    networks:
      - superpowers

  <svc>:
    network_mode: "service:<svc>-sidecar"
    # 删除原有的 ports: 和 networks:（由 sidecar 提供）
    environment:
      SERVER_HOST: "127.0.0.1"
      SERVER_PORT: "18080"
      # ... 其余环境变量不变
    depends_on:
      <svc>-sidecar:
        condition: service_started
```

- [ ] **Step 1: 改 docker-compose-services.yml — iam-service + sidecar**

将 iam-service 服务定义改为：

```yaml
  iam-sidecar:
    image: consul:1.15
    ports:
      - "6002:8080"
    command: >
      sh -c "until consul catalog services 2>/dev/null | grep -q iam-service; do echo 'waiting for iam-service registration...'; sleep 1; done
      && consul connect envoy -sidecar-for iam-service -grpc-addr=consul:8502 -admin-bind=0.0.0.0:19000"
    depends_on:
      consul:
        condition: service_healthy
    networks:
      - superpowers

  iam-service:
    build:
      context: ..
      dockerfile: services/iam-service/Dockerfile
    network_mode: "service:iam-sidecar"
    volumes:
      - ../docker-example/secrets:/app/keys:ro
      - ../services/iam-service/config:/etc/iam-service:ro
    environment:
      SERVER_HOST: "127.0.0.1"
      SERVER_PORT: "18080"
      DATABASE_HOST: mysql
      DATABASE_PORT: "3306"
      DATABASE_USER: root
      DATABASE_PASSWORD: rootpass
      DATABASE_NAME: superpowers
      CONSUL_ADDR: "http://consul:8500"
      CONFIG_PATH: "/etc/iam-service/config.yaml"
      OTEL_EXPORTER_OTLP_ENDPOINT: "http://otel-collector:4317"
    depends_on:
      mysql:
        condition: service_healthy
      redis:
        condition: service_healthy
      consul:
        condition: service_healthy
      iam-sidecar:
        condition: service_started
```

改动要点：
- 新增 `iam-sidecar` 容器（拥有网络栈，加入 superpowers network，端口映射 `6002:8080`）
- iam-service 删除 `ports` 和 `networks`，加 `network_mode: "service:iam-sidecar"`
- `SERVER_HOST` 改 `127.0.0.1`，`SERVER_PORT` 改 `18080`
- iam-outbox-publisher 保持不变（不接 HTTP，不走 mesh）

- [ ] **Step 2: 改 docker-compose-services.yml — 其余 6 个服务 + sidecar**

对 token-service、notification-service、audit-service、config-service、file-service、prue_backend、api-gateway 重复 Step 1 的模式。每个服务：

a. 新增 `<svc>-sidecar` 容器，端口映射保持原有 host port → sidecar 网络端口
b. app 容器删 `ports` + `networks`，加 `network_mode: "service:<svc>-sidecar"`
c. `SERVER_HOST` 改 `127.0.0.1`，`SERVER_PORT` 改 `18080`（config-service 原来 8081、file-service 原来 8082 也统一改 18080 — 内部端口统一，因为只监听 localhost 不冲突）
d. outbox-publisher 容器保持不变（不走 mesh）
e. frontend 容器保持不变

**端口映射对照表**（移到 sidecar 容器上）：

| 服务 | 原映射 | sidecar 映射 |
|------|--------|-------------|
| iam-service | 6002:8080 | 6002:8080 |
| token-service | 6001:8080 | 6001:8080 |
| notification-service | 6003:8080 | 6003:8080 |
| audit-service | 6005:8080 | 6005:8080 |
| config-service | 6007:8081 | 6007:8080 |
| file-service | 6006:8082 | 6006:8080 |
| prue_backend | 6004:8080 | 6004:8080 |
| api-gateway | 3000:3000 | 3000:3000 |

注意：config-service 和 file-service 的 sidecar 也统一用 8080 作为网络端口（app 内部端口统一为 18080）。需要同步改这两个服务的环境变量 `SERVER_PORT` 为 `18080`。

api-gateway 的 `GW_SERVER__PORT` 保持 3000（gateway 端口不变，sidecar 绑 3000）。

- [ ] **Step 3: 改 egress_map — api-gateway.toml**

将 `infrastructure/config/api-gateway/api-gateway.toml` 的 `[egress_map]` 改为：

```toml
[egress_map]
"iam-service"          = { host = "127.0.0.1", port = 9081 }
"token-service"        = { host = "127.0.0.1", port = 9082 }
"notification-service" = { host = "127.0.0.1", port = 9083 }
"audit-service"        = { host = "127.0.0.1", port = 9084 }
"prue_backend"         = { host = "127.0.0.1", port = 9087 }
"file-service"         = { host = "127.0.0.1", port = 9086 }
"config-service"       = { host = "127.0.0.1", port = 9085 }
```

- [ ] **Step 4: 改 token-service config — user_service_url 默认值**

在 `services/token-service/src/config.rs` 中，把 `user_service_url` 默认值从 `http://user-service:8080` 改为 `http://127.0.0.1:9081`：

```rust
            user_service_url: "http://127.0.0.1:9081".into(),
```

- [ ] **Step 5: 改 notification-service config — iam_base_url 默认值**

在 `services/notification-service/src/config.rs` 中，把 `default_iam_base_url()` 从 `http://iam-service:8080` 改为 `http://127.0.0.1:9081`：

```rust
fn default_iam_base_url() -> String {
    "http://127.0.0.1:9081".to_string()
}
```

- [ ] **Step 6: 改各服务 main.rs — ConsulRegistrar 加 upstreams**

6a. **api-gateway**（`services/api-gateway/src/main.rs`）— 需要全部 7 个 upstream：

找到 `ConsulRegistrar::new(...)` 调用处（约 line 300），改为：

```rust
    let mut consul_registrar = ConsulRegistrar::new(
        &cfg.consul_addr,
        "api-gateway",
        "/tmp/api-gateway-id",
    )
    .with_upstreams(vec![
        Upstream::new("iam-service", 9081),
        Upstream::new("token-service", 9082),
        Upstream::new("notification-service", 9083),
        Upstream::new("audit-service", 9084),
        Upstream::new("config-service", 9085),
        Upstream::new("file-service", 9086),
        Upstream::new("prue_backend", 9087),
    ]);
```

加 import：`use consul_registrar::Upstream;`（或确认已有的 import 路径）

6b. **token-service**（`services/token-service/src/main.rs`）— 需要 iam-service upstream：

找到 `ConsulRegistrar::new(...)` 调用处（约 line 158），改为：

```rust
    let mut consul_registrar = ConsulRegistrar::new(
        &cfg.consul.addr,
        &cfg.consul.service_name,
        "/tmp/token-service-id",
    )
    .with_upstreams(vec![
        Upstream::new("iam-service", 9081),
    ]);
```

加 import：`use consul_registrar::Upstream;`

6c. **notification-service**（`services/notification-service/src/main.rs`）— 需要 iam-service upstream：

找到 `ConsulRegistrar::new(...)` 调用处（约 line 149），改为：

```rust
    let mut consul_registrar = ConsulRegistrar::new(
        &cfg.consul.addr,
        &cfg.consul.service_name,
        "/tmp/notification-service-id",
    )
    .with_upstreams(vec![
        Upstream::new("iam-service", 9081),
    ]);
```

加 import：`use consul_registrar::Upstream;`

6d. **iam-service / audit-service / config-service / file-service / prue_backend** — 纯入站，无 upstream，只需确保 `ConsulRegistrar::new(...)` 不加 `with_upstreams`（空 Vec 即可，Connect sidecar 仍注册但无 upstream）。

这些服务的 main.rs **不需要改**——Task 1 的实现已经让 `new()` 默认 `upstreams: Vec::new()`，注册 payload 自动含 `Connect: { SidecarService: {} }`。

- [ ] **Step 7: 编译验证**

Run: `node scripts/run-tests-docker.js api-gateway token-service notification-service`
Expected: 编译通过（无 error）。测试可能因网络环境变化而需要调整，但编译必须通过。

- [ ] **Step 8: Commit**

```bash
git add infrastructure/docker-compose-services.yml infrastructure/config/api-gateway/api-gateway.toml services/token-service/src/config.rs services/notification-service/src/config.rs services/api-gateway/src/main.rs services/token-service/src/main.rs services/notification-service/src/main.rs
git commit -m "feat(mesh): service sidecar containers + localhost binding + upstream egress"
```

---

### Task 4: service-token 移除

**Files:**
- Delete: `services/iam-service/src/middleware/service_jwt.rs`
- Modify: `services/iam-service/src/middleware/mod.rs`
- Modify: `services/iam-service/src/routes/internal_users.rs`
- Modify: `services/iam-service/tests/internal_users_test.rs`

**Interfaces:**
- `/iam/internal/*` 端点不再挂 `require_service_jwt` 中间件
- handler 仍用 `Principal` extractor（含 `tenant_id`），确保 tenant 隔离
- mesh 意图替代服务身份认证（token-service / notification-service allow，其余 deny）
- `IamMemberClient` 保留 token 机制（仍需传 JWT 携带 `tid`，但不再要求 `aud: "service"`）

- [ ] **Step 1: 改 internal_users.rs — 移除中间件挂载**

在 `services/iam-service/src/routes/internal_users.rs` 中：

删除 import 行：
```rust
    middleware::service_jwt::require_service_jwt,
```

删除 `use axum::middleware;`（如果只有这一处用）。

把 `routes()` 函数改为：
```rust
pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/internal/users/by-role/{role_id}", get(list_users_by_role))
        .route("/iam/internal/users/by-dept/{dept_id}", get(list_users_by_dept))
        .with_state(state)
}
```

删除 `.route_layer(middleware::from_fn(require_service_jwt))` 行。

- [ ] **Step 2: 改 middleware/mod.rs**

将 `services/iam-service/src/middleware/mod.rs` 内容改为空模块：

```rust
```

（即删除 `pub mod service_jwt;` 行，文件留空或删文件并在上级移除引用——但保留空文件 `mod.rs` 避免其他引用断裂。）

- [ ] **Step 3: 删除 service_jwt.rs**

Run: `rm services/iam-service/src/middleware/service_jwt.rs`

- [ ] **Step 4: 改 internal_users_test.rs — 测试 fixture**

在 `services/iam-service/tests/internal_users_test.rs` 中：

4a. `mint_service_token` 不再需要 `aud: "service"`，改为只带 `tid` 的 token。将函数改为：

```rust
fn mint_internal_token(tid: i64) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    let header = URL_SAFE_NO_PAD.encode(b"{\"alg\":\"none\",\"typ\":\"JWT\"}");
    let payload = format!(r#"{{"tid":{tid},"sub":"svc"}}"#);
    let body = URL_SAFE_NO_PAD.encode(payload.as_bytes());
    format!("{header}.{body}.signature")
}
```

4b. 将所有 `mint_service_token` 调用改为 `mint_internal_token`。在 `setup()` 中：

```rust
    let service_token_t1 = mint_internal_token(1);
    let service_token_t2 = mint_internal_token(2);
```

4c. `Ctx` struct 的字段名可保留 `service_token_t1` / `service_token_t2`（避免大面积改名），或统一改为 `internal_token_t1` / `internal_token_t2`（preferred）。

4d. 更新文件头部注释：
```rust
//! Phase 4.4-fu2 + 4.3-fu2: tests for /iam/internal/users/by-{role,dept} endpoints.
//! Mesh intentions enforce service identity (Phase 5-A);
//! Principal extractor enforces tenant_id; cross-tenant reads forbidden.
```

- [ ] **Step 5: 运行测试**

Run: `node scripts/run-tests-docker.js iam-service`
Expected: 所有测试 PASS。`internal_users_test` 的 testcases 应全部通过——handler 仍用 `Principal` extractor 解析 `tid`，token 不再检查 `aud`。

- [ ] **Step 6: Commit**

```bash
git add services/iam-service/src/middleware/ services/iam-service/src/routes/internal_users.rs services/iam-service/tests/internal_users_test.rs
git commit -m "refactor(iam): remove require_service_jwt — mesh intentions replace service-token auth"
```

---

### Task 5: 全栈验证 + test-case 文档

**Files:**
- Create: `test-case/51-mtls-mesh.md`

- [ ] **Step 1: 全栈启动**

Run:
```bash
cd infrastructure
docker-compose -f docker-compose-infra.yml -f docker-compose-services.yml -f docker-compose-observability.yml up -d
sleep 30  # 等待所有 sidecar 注册 + envoy 启动
```

Expected: 所有容器 `healthy` 或 `running`。检查 sidecar：
```bash
docker-compose -f docker-compose-infra.yml -f docker-compose-services.yml ps | grep sidecar
```
所有 sidecar 容器应为 `running` 状态。

- [ ] **Step 2: 应用意图**

Run:
```bash
bash scripts/consul-intentions.sh
```
Expected: 输出 `Intentions applied successfully.` + `consul intention list` 列出全部 allow + deny 规则。

- [ ] **Step 3: e2e 回归**

Run:
```bash
node scripts/run-e2e-local.js
```
Expected: 全量 e2e 用例零失败。gateway → 下游服务通过 mesh sidecar mTLS 通信。

- [ ] **Step 4: 安全验证 — 非 mTLS 直连被拒**

Run:
```bash
# 从一个非 mesh 容器直接连 iam-service 的 sidecar 端口（无 mTLS）
docker run --rm --network superpowers alpine/curl \
  curl -s -o /dev/null -w "%{http_code}" http://iam-sidecar:8080/health
```
Expected: 连接失败或非 200（Consul Connect 入站 sidecar 拒绝无 mTLS 的连接）。

- [ ] **Step 5: 意图验证 — 非授权服务被拒**

Run:
```bash
# file-service 尝试调 iam-service（意图 deny）
docker exec infrastructure-file-service-1 wget -q -O- http://127.0.0.1:9081/health 2>&1 || true
```
Expected: 连接被拒（file-service 无 iam-service 的 upstream，本地无 9081 监听；即使有，意图 deny 会阻止 mTLS 握手）。

- [ ] **Step 6: 创建 test-case 文档**

创建 `test-case/51-mtls-mesh.md`：

```markdown
# 51 — Consul Connect 服务网格验收

> Phase 5-A: mTLS + 意图 + mesh 可观测性

## 前置

- `docker-compose -f docker-compose-infra.yml -f docker-compose-services.yml -f docker-compose-observability.yml up -d`
- `bash scripts/consul-intentions.sh`

## 验收步骤

### 1. Sidecar 全部就绪

```bash
docker-compose -f docker-compose-infra.yml -f docker-compose-services.yml ps | grep sidecar
```
所有 `<svc>-sidecar` 容器 `running`。

### 2. e2e 全量回归

```bash
node scripts/run-e2e-local.js
```
零失败。gateway → 下游通过 mesh mTLS。

### 3. 非 mTLS 直连被拒

```bash
docker run --rm --network superpowers alpine/curl \
  curl -s -o /dev/null -w "%{http_code}" http://iam-sidecar:8080/health
```
非 200 / 连接拒绝。

### 4. 意图 deny 生效

file-service 无 iam-service 的 upstream（本地无 9081 监听），无法发起调用。

### 5. Mesh metrics 可观测

Consul UI (`http://localhost:8500`) → Connect → 查看 sidecar proxy 状态。
Prometheus (`http://localhost:9090`) → 查询 `envoy_cluster_upstream_cx_connect_fail` 应为 0。
```

- [ ] **Step 7: Commit**

```bash
git add test-case/51-mtls-mesh.md
git commit -m "docs(test-case): Phase 5-A mTLS mesh acceptance checklist"
```

---

## Self-Review 记录

**Spec coverage 检查:**

| Spec 章节 | 对应 Task | 状态 |
|-----------|-----------|------|
| §1 架构与拓扑 | Task 2 (Connect 启用) + Task 3 (sidecar) | ✅ |
| §2 证书与服务身份 | Task 1 (注册 connect 字段) + Task 2 (CA) | ✅ — 证书轮换自动，无需代码 |
| §3 意图矩阵 | Task 2 (意图脚本) + Task 4 (service-token 移除) | ✅ |
| §4 迁移步骤 1-6 | Task 1→2→3→4→5 | ✅ — 一次性切换 |
| §5 可观测性与测试 | Task 5 (验证 + test-case) | ✅ |

**Placeholder 扫描:** 无 TBD/TODO。Step 中有 "约 line N" 引用——实施时用 grep 确认行号即可。

**类型一致性:** `Upstream::new(name, port)` 在 Task 1 定义，Task 3 使用——签名一致。`with_upstreams(Vec<Upstream>)` 一致。

**已知风险（实施时注意）:**
1. `network_mode: "service:X"` 下 app 容器不能用 `ports`/`networks` — 已在 Step 说明
2. sidecar entrypoint 的 `until consul catalog services | grep` 等待循环 — 如果 app 注册失败，sidecar 永久等待（开发环境可接受，加 `timeout 120` 兜底）
3. config-service / file-service 端口从 8081/8082 统一到 18080 — sidecar 网络端口也统一到 8080，原有 host port 映射不变
4. `IamMemberClient` 保留 token 机制 — spec 说"删 NOTIF_SERVICE_TOKEN_INITIAL"，但实际仍需 JWT 传 tid；plan 保留 token 机制，只是不再要求 `aud: "service"`
