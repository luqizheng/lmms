# Phase 5-A mTLS Sidecar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans 实施本计划。Steps use checkbox (`- [ ]`) 语法跟踪。

> ⚠️ **Superseded by** [`docs/superpowers/plans/2026-07-07-deprecate-consul-connect.md`](2026-07-07-deprecate-consul-connect.md) (2026-07-07). The consul-dataplane sidecar approach described here was rolled back. Retained for historical reference only.

**Goal:** 为 docker-compose 下每个业务服务接入 Consul Connect mTLS sidecar（consul-dataplane），并删除应用层 `require_service_token` 中间件。

**Architecture:** Consul 1.15 启用内置 Connect CA → 服务注册时声明 `connect.sidecar_service` → 每个业务容器配 `hashicorp/consul-dataplane:1.2` 伴生容器，业务容器通过 `network_mode: service:<svc>-sidecar` 共享网络命名空间；sidecar ingress 在 :20000 终止上游 mTLS 转发到业务进程，egress 为**每个上游分配独立的 `local_bind_port`**，gateway 按服务名映射本机端口建立连接。

**Tech Stack:** Consul 1.15 (Connect CA 内置) + consul-dataplane 1.2 + docker-compose `network_mode: service` + Axum 0.8 router 重构。

## Global Constraints

- Spec 来源：`docs/superpowers/specs/2026-06-29-phase5a-mtls-sidecar-design.md`。
- Sidecar 镜像锁定 `hashicorp/consul-dataplane:1.2`；Consul 镜像锁定 `consul:1.15`。
- **本计划仅覆盖 docker-compose-services.yml 中实际部署的 HTTP 业务服务**：`iam-service` / `token-service` / `notification-service` / `audit-service` / `prue_backend`（5 个下游 + `api-gateway` 1 个上游）。`config-service` / `file-service` 源码已存在但未在 compose 中部署，**不在本计划范围**；后续部署时按相同模式补 sidecar。`iam-outbox-publisher` / `notification-outbox-publisher` 无 HTTP listener，**不配 sidecar**（mTLS 仅保护 HTTP 服务间调用，outbox publisher 只访问 mysql/rabbitmq）。
- 共享网络命名空间规则：使用 `network_mode: service:<svc>-sidecar` 后，业务容器 **不得**自己声明 `ports:`，**所有对外端口由 sidecar 容器声明**。
- Sidecar ingress 端口固定 `20000`（业务端口由 sidecar 透传给业务容器本地的 `localhost:8080`）。
- Gateway egress per-upstream `local_bind_port` 映射（写死，便于排障）：
  - `iam-service` → `localhost:18001`
  - `token-service` → `localhost:18002`
  - `notification-service` → `localhost:18003`
  - `audit-service` → `localhost:18004`
  - `prue_backend` → `localhost:18005`
- Consul 服务注册：本计划之前 `iam-service` / `token-service` 同名服务**双重注册**（static config + ConsulRegistrar 各注册一次）。本计划精简：static config 只保留 `api-gateway` + `prue_backend`（不调 ConsulRegistrar 的 2 个），其余 4 个 HTTP 服务（iam/token/notification/audit）统一走 `libs/consul-registrar` 注册路径自动带 Connect 字段（Task 4 修改 lib 一处覆盖全部 4 服务）。这与 AGENTS.md "Rust 服务统一 Consul 注册" 原则一致。Sidecar proxy id 固定为 `{service_name}-sidecar-proxy`（不带 uuid），匹配 sidecar 容器的 `DP_PROXY_SERVICE_ID` 环境变量。
- **关键安全约束（4A）**：所有业务进程**必须**绑定 `127.0.0.1`（在 sidecar netns 内回环监听），不能绑 `0.0.0.0`。否则同 docker 网络的其他容器可直连业务端口（如 `iam-sidecar:8080`）**绕过 sidecar mTLS**，使整个 mTLS 部署形同虚设。
  - iam/token/audit/prue_backend：env `SERVER_HOST: "127.0.0.1"`（覆盖 AGENTS.md 默认 `0.0.0.0`）
  - notification-service：用配置文件 `http_addr`（不响应 `SERVER_HOST` env），改 `docker-example/config/notification-service/config.yaml` 的 `server.http_addr: "127.0.0.1:8080"`
  - api-gateway 例外：是系统边界，仍 `GW_SERVER__HOST: 0.0.0.0` 接受外部明文 :3000
  - Healthcheck（如 token-service `wget http://127.0.0.1:8080/.well-known/jwks.json`）在 sidecar netns 内 exec 仍能命中，**不受影响**
- 应用层 `require_service_token` 当前**零调用方**（`grep -rn require_service_token services/ --include='*.rs'` 返回空）—— 仅需删除 `libs/digua_authz/src/require_service_token.rs` + 单测 + `libs/digua_authz/src/lib.rs` 两行导出。无需 sed 服务侧。
- Gateway 转发逻辑变更：`router::forward_to_service` URL 拼接从 `http://{consul_address}:{consul_port}{path}` 改为 `http://{egress_host}:{egress_port}{path}`，其中 `(egress_host, egress_port)` 由 `RouterConfig.egress_map: HashMap<String, (String, u16)>` 按 `service_name` 查表。
- 服务发现：旧 `discovery::ServiceDiscovery` 在 Task 5 后**完全删除**——sidecar envoy 自己维护下游健康端点（EDS），上层再 ping Consul 是重复 RTT + 误报信号源（Consul 抖动 ≠ 下游故障）。健康信号路径变为：sidecar 连接失败 → `ResilientClient::call` 返 `Connect`/`Timeout` → 喂熔断。
- 测试基线：`node scripts/run-e2e-local.js` 全量通过。
- 每个 task 末尾 commit，commit 信息前缀 `P5-A:`。
- 商业服务不引入新的 DB 表，无 migration。

---

## File Structure

| 文件 | 责任 |
|------|------|
| `infrastructure/config/consul/connect.hcl` | **新增**：`connect { enabled = true }`，挂入 `/consul/config/`，避免内联 HCL 解析陷阱 |
| `infrastructure/docker-compose-infra.yml` | Consul `volumes` 加挂 connect.hcl；`ports` 暴露 :8502 用于排障；`healthcheck` 同时探活 HTTP + gRPC |
| `docker-example/config/consul/config.docker.json` | 精简为仅 `api-gateway` + `prue_backend` 静态注册（去掉 iam/token 重复），加 `connect.sidecar_service`；api-gateway 声明 5 个 upstreams |
| `libs/consul-registrar/src/lib.rs` | `AgentServiceRegistration` 加 `connect: ConnectRegistration` 字段，所有调用方（iam/token/notification/audit）零改动自动注册 sidecar_service |
| `infrastructure/docker-compose-services.yml` | 新增 6 个 `<svc>-sidecar` service；为 6 个业务服务改 `network_mode: service:<svc>-sidecar`，移除业务容器自身 `ports:` |
| `infrastructure/sidecar/README.md` | sidecar 模板与端口规约文档 |
| `services/api-gateway/src/discovery.rs` | **删除**（mTLS 后 sidecar envoy 维护下游健康端点，gateway 不再自查 Consul） |
| `services/api-gateway/src/lib.rs` | 删除 `pub mod discovery;` 与 `pub use discovery::*;` |
| `services/api-gateway/src/config.rs` | `GatewayConfig` 新增 `egress_map: HashMap<String, EgressTarget>` |
| `services/api-gateway/src/router.rs` | `RouterConfig` 移除 `discovery` 字段，加 `egress_map`，`forward_to_service` 改用 egress 拼 URL |
| `services/api-gateway/src/main.rs` | 启动时构造 `egress_map`（默认值 + 环境变量覆盖），移除 `ServiceDiscovery::new` |
| `services/notification-service/src/main.rs` | **不改动**（Task 4 走 lib 自动获得 sidecar 注册）|
| `services/audit-service/src/main.rs` | **不改动**（同上） |
| `libs/digua_authz/src/lib.rs` | 删除 `pub mod require_service_token;` 与 `pub use require_service_token::require_service_token;` 两行 |
| `libs/digua_authz/src/require_service_token.rs` | 删除 |
| `libs/digua_authz/tests/require_service_token_test.rs` | 删除 |
| `docs-site/guides/mtls-deployment.md` | 运维手册（CA 备份、轮换、回滚、故障排查） |
| `TODO.md` | Phase 5-A 段勾选完成 |

---

## Task 1: 启用 Consul Connect CA

**Files:**
- Create: `infrastructure/config/consul/connect.hcl`
- Modify: `infrastructure/docker-compose-infra.yml`

**Interfaces:**
- Produces: 一个可用的 Connect CA + 已就绪的 gRPC :8502，curl `http://localhost:8500/v1/connect/ca/roots` 返回非空 `Roots[]`，`nc -z localhost 8502` 成功。

- [ ] **Step 1: 新建 HCL 配置文件**

```bash
mkdir -p infrastructure/config/consul
```

写入 `infrastructure/config/consul/connect.hcl`：

```hcl
connect {
  enabled = true
}
```

> 用 HCL 文件而非 `-hcl=...` 内联 flag：Consul 1.15 CLI 的 `-hcl` 对空格 / 引号转义敏感（`connect{enabled=true}` 紧凑写法在某些 shell + compose 组合下解析失败），文件挂载更稳定。

- [ ] **Step 2: 修改 consul service**

将 `infrastructure/docker-compose-infra.yml` 的 `consul:` 段替换为：

```yaml
  consul:
    image: consul:1.15
    ports:
      - "8500:8500"
      - "8502:8502"
    volumes:
      - ../docker-example/config/consul/config.docker.json:/consul/config/services.json:ro
      - ./config/consul/connect.hcl:/consul/config/connect.hcl:ro
    command: agent -dev -config-dir=/consul/config -bind=0.0.0.0 -client=0.0.0.0
    healthcheck:
      test: ["CMD-SHELL", "consul members && nc -z 127.0.0.1 8502"]
      interval: 10s
      timeout: 5s
      retries: 5
    networks:
      - superpowers
```

变化点：
- `ports` 加 `"8502:8502"` —— 暴露 gRPC 给 host，方便排障（`grpcurl -plaintext localhost:8502 list`）；不影响安全（dev 模式本身明文）
- `volumes` 多挂 `connect.hcl`（`-config-dir` 已包含 `/consul/config/`，文件自动加载）
- `healthcheck` 同时验证 HTTP API + gRPC 端口可达；`nc` 来自 consul 镜像的 alpine base（已自带 busybox `nc`）

- [ ] **Step 3: 启动并验证 CA + gRPC**

```bash
docker compose -f infrastructure/docker-compose-infra.yml down
docker compose -f infrastructure/docker-compose-infra.yml up -d consul
sleep 8
# 验证 1：CA 已生成
curl -fsS http://localhost:8500/v1/connect/ca/roots | jq '.Roots | length'
# 验证 2：gRPC 端口可达
nc -zv localhost 8502 2>&1 | grep -i "succeeded\|open"
# 验证 3：容器 healthy
docker compose -f infrastructure/docker-compose-infra.yml ps consul | grep -i healthy
```

Expected:
- `1` (至少一个根 CA)
- `Connection to localhost 8502 ... succeeded!` (或 `open`)
- consul 状态 `Up (healthy)`

- [ ] **Step 4: Commit**

```bash
git add infrastructure/config/consul/connect.hcl infrastructure/docker-compose-infra.yml
git commit -m "P5-A: enable Consul Connect CA + expose gRPC 8502 with healthcheck"
```

---

## Task 2: Sidecar 模板与端口规约文档

**Files:**
- Create: `infrastructure/sidecar/README.md`

**Interfaces:**
- Produces: 后续 task 引用本文件作为 sidecar 配置规范来源。

- [ ] **Step 1: 创建目录与文档**

```bash
mkdir -p infrastructure/sidecar
```

写入 `infrastructure/sidecar/README.md`：

```markdown
# Consul Dataplane Sidecar 模板

> Phase 5-A mTLS 服务网格规约。所有业务服务伴生容器统一模板。

## Ingress 端口（业务容器对外入口）

每个 sidecar 暴露固定 `20000`，sidecar 自动把解密后的明文转给 `localhost:<业务端口>`。
业务容器本身仍监听 `0.0.0.0:8080`（gateway 是 3000），但**不**通过 docker port 映射对外，
由 sidecar 的 `ports:` 段做端口暴露。

| Service             | 业务端口 | sidecar ports                | 外部访问端口（host） |
|---------------------|----------|------------------------------|----------------------|
| iam-service         | 8080     | "6002:20000"                 | 6002                 |
| token-service       | 8080     | "6001:20000"                 | 6001                 |
| notification-service| 8080     | "6003:20000"                 | 6003                 |
| audit-service       | 8080     | "6005:20000"                 | 6005                 |
| prue_backend        | 8080     | "6004:20000"                 | 6004                 |
| api-gateway         | 3000     | "3000:3000" (业务进程明文)   | 3000                 |

> **gateway 例外**：api-gateway 是系统**边界**，对外接受用户 HTTP/JWT，无需对外 mTLS。所以
> gateway-sidecar **不开 ingress listener**（不要 :20000），只做 egress（5 个 upstream listener）。
> Host:3000 直接映射到业务进程的 3000，不经 envoy。

## Egress 端口（上游侧 sidecar）

仅 `api-gateway` 需要 egress，按下表声明 `proxy.upstreams[].local_bind_port`：

| 上游 destination_name | local_bind_port |
|-----------------------|-----------------|
| iam-service           | 18001           |
| token-service         | 18002           |
| notification-service  | 18003           |
| audit-service         | 18004           |
| prue_backend          | 18005           |

## Sidecar Compose 模板

声明在 `infrastructure/docker-compose-services.yml` 顶部的 anchors：

\`\`\`yaml
x-sidecar-defaults: &sidecar-defaults
  image: hashicorp/consul-dataplane:1.2
  mem_limit: 64m
  depends_on:
    consul:
      condition: service_healthy
  networks:
    - superpowers

x-sidecar-env: &sidecar-env
  DP_CONSUL_ADDRESSES: consul
  DP_CONSUL_GRPC_PORT: "8502"
  DP_ENVOY_READY_BIND_ADDR: 0.0.0.0
  DP_ENVOY_READY_BIND_PORT: "20100"
  DP_ENVOY_CONCURRENCY: "1"
\`\`\`

每个 sidecar 服务：

\`\`\`yaml
  <svc>-sidecar:
    <<: *sidecar-defaults
    environment:
      <<: *sidecar-env
      DP_PROXY_SERVICE_ID: <service-name>-sidecar-proxy
    ports: ["<外部端口>:20000"]    # gateway 例外: "3000:3000"
\`\`\`

业务容器：

\`\`\`yaml
  <svc>:
    network_mode: service:<svc>-sidecar
    # 删除自身 ports:，由 sidecar 暴露
    depends_on:
      <svc>-sidecar:
        condition: service_started
\`\`\`

## 注意事项

- `network_mode: service:X` 后业务容器**不能**再写 `networks:` 或 `ports:`。
- Consul gRPC 端口 8502 在 dev mode 默认启用；如自建集群需 `-grpc-port 8502` 显式开。
- Sidecar 启动需要 Consul 已注册对应 service + sidecar_service 定义，否则 envoy 无法配置 listener。
- **DNS 行为**：业务容器使用 `network_mode: service:<svc>-sidecar` 后**没有自己的网络栈**，docker 内部 DNS 对服务名 `<svc>` 的解析会返回 **sidecar 容器的 IP**（因为只有 sidecar 容器加入了 `superpowers` network）。这是正常工作模式：其他容器（如 `frontend`）通过 `http://api-gateway:3000` 仍能连到 gateway 业务进程，路径是 `frontend → gateway-sidecar netns → :3000 (gateway process)`。任何排障时如发现"DNS 解析到 sidecar IP" 属于预期行为，**不要**改动。
```

- [ ] **Step 2: Commit**

```bash
git add infrastructure/sidecar/README.md
git commit -m "P5-A: document sidecar template and port allocation"
```

---

## Task 3: Consul 静态注册声明 sidecar_service（仅 api-gateway + prue_backend）

**Files:**
- Modify: `docker-example/config/consul/config.docker.json`

**Interfaces:**
- Consumes: Task 1 已启用 Connect。
- Produces: 启动 Consul 即注册 `api-gateway` 与 `prue_backend` 的 sidecar_service；api-gateway 注册声明全部 5 个 upstreams。

> **注册路径统一**（仅本计划范围内的决策）：本计划之前 `iam-service` / `token-service` 同时存在 static config + ConsulRegistrar 双重注册（同名 service 注册两次，id 不同）。本 task 把静态条目精简为**仅** `api-gateway` 与 `prue_backend`（这两个服务不调用 `libs/consul-registrar`），其余 4 个 HTTP 服务（iam/token/notification/audit）统一走 Task 4 的 ConsulRegistrar 路径自动带 Connect。这与 AGENTS.md 中的"Rust 服务统一 Consul 注册"原则一致。

- [ ] **Step 1: 替换文件全文**

将 `docker-example/config/consul/config.docker.json` 整文件替换为：

```json
{
  "services": [
    {
      "name": "prue_backend",
      "id": "prue_backend",
      "address": "prue_backend",
      "port": 8080,
      "check": {
        "name": "http",
        "http": "http://prue_backend:8080/health",
        "interval": "10s",
        "timeout": "5s"
      },
      "connect": {
        "sidecar_service": {
          "id": "prue_backend-sidecar-proxy",
          "name": "prue_backend-sidecar-proxy"
        }
      }
    },
    {
      "name": "api-gateway",
      "id": "api-gateway",
      "address": "api-gateway",
      "port": 3000,
      "check": {
        "name": "http",
        "http": "http://api-gateway:3000/health",
        "interval": "10s",
        "timeout": "5s"
      },
      "connect": {
        "sidecar_service": {
          "id": "api-gateway-sidecar-proxy",
          "name": "api-gateway-sidecar-proxy",
          "proxy": {
            "upstreams": [
              { "destination_name": "iam-service",          "local_bind_port": 18001 },
              { "destination_name": "token-service",        "local_bind_port": 18002 },
              { "destination_name": "notification-service", "local_bind_port": 18003 },
              { "destination_name": "audit-service",        "local_bind_port": 18004 },
              { "destination_name": "prue_backend",         "local_bind_port": 18005 }
            ]
          }
        }
      }
    }
  ]
}
```

> 关键变化：
> - 删除 `token-service` / `iam-service` 静态条目（改由 ConsulRegistrar 注册，Task 4）
> - 显式声明 `id` 与 `sidecar_service.id` / `sidecar_service.name`：sidecar 容器的 `DP_PROXY_SERVICE_ID=<svc>-sidecar-proxy` 是固定值，不依赖 Consul 自动派生（自动派生格式是 `{parent_id}-sidecar-proxy`，ConsulRegistrar 用 `{name}-{uuid}` 时会失配）

- [ ] **Step 2: 重启 consul 验证注册**

```bash
docker compose -f infrastructure/docker-compose-infra.yml restart consul
sleep 4
curl -fsS http://localhost:8500/v1/agent/services | jq 'keys'
```

Expected: 输出包含 `"api-gateway"`, `"api-gateway-sidecar-proxy"`, `"prue_backend"`, `"prue_backend-sidecar-proxy"` 共 4 个 service id；`iam-service` / `token-service` / `notification-service` / `audit-service` 此时**不出现**（它们将在 Task 4 通过 ConsulRegistrar 在服务启动时注册）。

- [ ] **Step 3: Commit**

```bash
git add docker-example/config/consul/config.docker.json
git commit -m "P5-A: declare sidecar_service for static-registered services; remove duplicates"
```

---

## Task 4: libs/consul-registrar 注入 Connect 字段（自动覆盖 4 服务）

**Files:**
- Modify: `libs/consul-registrar/src/lib.rs`

**Interfaces:**
- Consumes: Task 1。
- Produces: `AgentServiceRegistration` 序列化时自带 `connect.sidecar_service.id / name` 字段。所有调用方（iam-service / token-service / notification-service / audit-service）零代码改动自动获得 sidecar 注册。

> **架构决策**：iam/token/notification/audit 4 个服务都用 `libs/consul-registrar::ConsulRegistrar`，注册体由共享 struct `AgentServiceRegistration` 序列化（`libs/consul-registrar/src/lib.rs:19-25`）。在 lib 层加 `connect` 字段 = 一处改动覆盖 4 个服务，符合"DRY 优先"和 AGENTS.md 的"统一 Consul 注册"原则。
>
> **sidecar id 规约**：sidecar_service.id 必须是固定的 `{service_name}-sidecar-proxy`（不带 uuid），以匹配 Task 6-12 中 sidecar 容器的 `DP_PROXY_SERVICE_ID` 环境变量。parent service id 可以带 uuid（ConsulRegistrar 现有行为不变）。

- [ ] **Step 1: grep 确认所有调用方**

```bash
grep -rln "ConsulRegistrar\|consul_registrar" services/ --include='*.rs'
```

Expected: 应只命中 `services/iam-service/src/main.rs`、`services/token-service/src/{main.rs,lib.rs}`、`services/notification-service/src/main.rs`、`services/audit-service/src/main.rs`。若有其他命中，停下评估改动影响范围。

- [ ] **Step 2: 在 lib.rs 加 Connect 序列化结构**

修改 `libs/consul-registrar/src/lib.rs`。在 `AgentServiceCheck` 定义（约 line 27-33）之后追加：

```rust
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct ConnectRegistration {
    sidecar_service: SidecarService,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct SidecarService {
    id: String,
    name: String,
}
```

> Consul HTTP API 注册端点（`PUT /v1/agent/service/register`）字段为 PascalCase（与读端 `GET /v1/agent/services` 返回的字段一致）。`#[serde(rename_all = "PascalCase")]` 把 Rust snake_case 字段名映射为 `SidecarService` / `Id` / `Name`。

- [ ] **Step 3: 在 AgentServiceRegistration 加 connect 字段**

修改 `AgentServiceRegistration` struct（line 18-25），改为：

```rust
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct AgentServiceRegistration {
    id: String,
    name: String,
    address: String,
    port: u16,
    check: AgentServiceCheck,
    connect: ConnectRegistration,
}
```

> 改动点：
> 1. 加 `#[serde(rename_all = "PascalCase")]` —— 现有 struct **没**带 rename attr，依赖 Consul 接受 lowercase（实测可工作但非官方文档保证）。本 task 显式统一为 PascalCase，与新加的 `ConnectRegistration` 一致。
> 2. 新增 `connect: ConnectRegistration` 字段（不是 `Option`——本项目所有 ConsulRegistrar 用户都需要 sidecar，不留 opt-out 渠道；未来如有需要再扩为 builder）。

- [ ] **Step 4: 构造 `connect` 字段**

修改 `register()` 方法（约 line 226-251），在 `let registration = AgentServiceRegistration { ... }` 处加 `connect` 字段：

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
            connect: ConnectRegistration {
                sidecar_service: SidecarService {
                    id: format!("{}-sidecar-proxy", self.service_name),
                    name: format!("{}-sidecar-proxy", self.service_name),
                },
            },
        };
```

> 关键：`sidecar_service.id` 用 `self.service_name`（如 `audit-service`），不是 `self.service_id`（带 uuid）。这样 sidecar 容器的 `DP_PROXY_SERVICE_ID=audit-service-sidecar-proxy` 始终能在 Consul 找到。

- [ ] **Step 5: AgentServiceCheck 同步 PascalCase**

`AgentServiceCheck` struct 也加 attr，保持一致：

```rust
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct AgentServiceCheck {
    http: String,
    interval: String,
    timeout: String,
    #[serde(rename = "DeregisterCriticalServiceAfter")]
    deregister_critical_service_after: String,
}
```

> `Http` 在 Consul HTTP API 中是 `HTTP`（全大写），需 `#[serde(rename = "HTTP")]`。实测 Consul 也接受 `Http`，但 [HashiCorp 官方 doc](https://developer.hashicorp.com/consul/api-docs/agent/service#http) 用 `HTTP`，更稳。这里改为：

```rust
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct AgentServiceCheck {
    #[serde(rename = "HTTP")]
    http: String,
    interval: String,
    timeout: String,
    deregister_critical_service_after: String,
}
```

> 注：`#[serde(rename_all = "PascalCase")]` 把 `deregister_critical_service_after` 自动映射为 `DeregisterCriticalServiceAfter`（PascalCase 把每个下划线段首字母大写），无需再加单独 rename。

- [ ] **Step 6: 单测：序列化字段验证 + 集成测试 wiremock 校验 register payload**

> 两层覆盖：序列化单测锁定字段名/值；wiremock 集成测试模拟 Consul 收 PUT 请求并断言完整 payload，防止"struct 单测过但实际网络发出的字节有别"的回归（如 reqwest 中间件改写、http_client 配置漂移）。

在 `libs/consul-registrar/Cargo.toml` 的 `[dev-dependencies]` 段加（如不存在则新建该段）：

```toml
wiremock = { workspace = true }
```

如 workspace `Cargo.toml` 未声明 `wiremock`，从 `libs/digua_common-config/Cargo.toml`（已用 `wiremock = "0.6"`）借同版本，或在 workspace dependencies 加 `wiremock = "0.6"`。

在 `libs/consul-registrar/src/lib.rs` 的 `#[cfg(test)] mod tests` 末尾追加：

```rust
    #[test]
    fn registration_serializes_connect_sidecar_with_fixed_id() {
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
            connect: ConnectRegistration {
                sidecar_service: SidecarService {
                    id: "audit-service-sidecar-proxy".into(),
                    name: "audit-service-sidecar-proxy".into(),
                },
            },
        };
        let json = serde_json::to_value(&reg).unwrap();
        assert_eq!(json["Connect"]["SidecarService"]["ID"], "audit-service-sidecar-proxy");
        assert_eq!(json["Connect"]["SidecarService"]["Name"], "audit-service-sidecar-proxy");
        assert_eq!(json["Check"]["HTTP"], "http://127.0.0.1:8080/health");
        assert_eq!(json["Check"]["DeregisterCriticalServiceAfter"], "1m");
        assert_eq!(json["ID"], "audit-service-abc-uuid");
        assert_eq!(json["Name"], "audit-service");
    }
```

新建 `libs/consul-registrar/tests/register_payload_test.rs`：

```rust
//! 集成测试：用 wiremock 模拟 Consul agent，截获 PUT /v1/agent/service/register
//! 实际发出的字节，验证 Connect.SidecarService.ID 使用 service_name（不带 uuid）。

use serde_json::Value;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

#[tokio::test]
async fn register_payload_includes_connect_sidecar_with_fixed_id() {
    let server = MockServer::start().await;
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None::<Value>));
    let cap_clone = captured.clone();

    Mock::given(method("PUT"))
        .and(path("/v1/agent/service/register"))
        .respond_with(move |req: &Request| {
            let body: Value = serde_json::from_slice(&req.body).expect("body json");
            *cap_clone.lock().unwrap() = Some(body);
            ResponseTemplate::new(200)
        })
        .mount(&server)
        .await;

    let tmp = tempfile::tempdir().unwrap();
    let id_path = tmp.path().join("svc-id").to_string_lossy().to_string();
    let mut r = consul_registrar::ConsulRegistrar::new(&server.uri(), "audit-service", &id_path);
    let _ = r.register("127.0.0.1", 8080).await;

    let body = captured.lock().unwrap().clone().expect("register not called");
    assert_eq!(body["Connect"]["SidecarService"]["ID"], "audit-service-sidecar-proxy");
    assert_eq!(body["Connect"]["SidecarService"]["Name"], "audit-service-sidecar-proxy");
    assert_eq!(body["Name"], "audit-service");
    assert!(
        body["ID"].as_str().unwrap().starts_with("audit-service-"),
        "parent id should keep uuid suffix, got {}",
        body["ID"]
    );
}
```

> 这测试验证三件事：① 实际网络发出的 payload 含 Connect.SidecarService（不光是 struct 单测）；② sidecar id 用 service_name 不含 uuid；③ parent service id 保留 uuid 后缀（ConsulRegistrar 现有行为不退化）。

- [ ] **Step 7: 编译 + 单测**

```bash
cargo build -p consul-registrar
cargo test -p consul-registrar
```

Expected: 全部通过（含新加的 `registration_serializes_connect_sidecar_with_fixed_id` 单测 + `register_payload_includes_connect_sidecar_with_fixed_id` wiremock 集成测试 + 现有的 `register_persists_id_even_when_consul_unreachable` 等）。

- [ ] **Step 8: 联编验证下游服务不受字段变化影响**

```bash
cargo build -p iam-service -p token-service -p notification-service -p audit-service
```

Expected: 无错误。`AgentServiceRegistration` 是 `libs/consul-registrar/src/lib.rs` 内部 struct（无 `pub`），下游服务只通过 `ConsulRegistrar` API 交互，字段变化对它们透明。

- [ ] **Step 9: Commit**

```bash
git add libs/consul-registrar/
git commit -m "P5-A: consul-registrar emits Connect.SidecarService with fixed proxy id"
```

---

## Task 5: Gateway router 改走 sidecar egress（删除 ServiceDiscovery）

**Files:**
- Modify: `services/api-gateway/src/config.rs`
- Modify: `services/api-gateway/src/router.rs`
- Modify: `services/api-gateway/src/main.rs`
- Modify: `services/api-gateway/src/lib.rs`
- Delete: `services/api-gateway/src/discovery.rs`

**Interfaces:**
- Consumes: Task 3 声明的 upstream `local_bind_port`。
- Produces: `RouterConfig` 字段简化为 `{ resilient, egress_map }`（去掉 discovery）；`forward_to_service` 用 `egress_map.get(service_name)` 决定连接目标；连接失败 / 超时由 sidecar 反映到 `ResilientClient` 喂熔断。

> **决策**：mTLS 后 sidecar envoy 自己用 EDS 维护下游健康端点，gateway 再 ping Consul 是冗余 RTT + 误报源（Consul 抖动 ≠ 下游故障）。删除 `ServiceDiscovery`，健康信号完全由 `ResilientClient` 接管。先 `grep -rn ServiceDiscovery services/ libs/` 确认仅 api-gateway 内部使用（应只命中 `services/api-gateway/src/{discovery.rs,lib.rs,main.rs,router.rs}`）。

- [ ] **Step 1: 在 config.rs 加 EgressTarget**

在 `services/api-gateway/src/config.rs` 文件末尾追加：

```rust
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct EgressTarget {
    pub host: String,
    pub port: u16,
}

fn default_egress_map() -> std::collections::HashMap<String, EgressTarget> {
    let mut m = std::collections::HashMap::new();
    let local = |p: u16| EgressTarget { host: "127.0.0.1".into(), port: p };
    m.insert("iam-service".into(),          local(18001));
    m.insert("token-service".into(),        local(18002));
    m.insert("notification-service".into(), local(18003));
    m.insert("audit-service".into(),        local(18004));
    m.insert("prue_backend".into(),         local(18005));
    m
}
```

在 `GatewayConfig` struct 内追加字段：

```rust
    #[serde(default = "default_egress_map")]
    pub egress_map: HashMap<String, EgressTarget>,
```

`Default for GatewayConfig` 的 fields 末尾追加：

```rust
            egress_map: default_egress_map(),
```

- [ ] **Step 2: 修改 router.rs 接入 egress_map（删除 discovery 依赖）**

将 `services/api-gateway/src/router.rs` 整文件替换为：

```rust
use crate::config::EgressTarget;
use axum::body::Body;
use axum::http::Request;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use digua_common_http::{ResilientClient, ResilientError};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct RouterConfig {
    pub resilient: Arc<ResilientClient>,
    pub egress_map: Arc<HashMap<String, EgressTarget>>,
}

/// 通过 sidecar egress 转发：URL 目标取自 `egress_map[service_name]`，
/// 由 sidecar 完成 mTLS 与对端选择。下游健康由 sidecar 的 EDS 维护，
/// 连接失败 / 超时透过 `ResilientClient` 喂熔断（决策 9A/14B 适用）。
pub async fn forward_to_service(
    cfg: Arc<RouterConfig>,
    request: Request<Body>,
    service_name: &str,
    target_path_query: &str,
) -> Response {
    let Some(egress) = cfg.egress_map.get(service_name) else {
        tracing::error!("no egress configured for service '{}'", service_name);
        return StatusCode::BAD_GATEWAY.into_response();
    };

    let target_url = format!(
        "http://{}:{}{}",
        egress.host, egress.port, target_path_query
    );

    let (parts, body) = request.into_parts();
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    match cfg
        .resilient
        .call(service_name, parts.method, &target_url, &parts.headers, bytes)
        .await
    {
        Ok(resp) => {
            let mut builder = Response::builder().status(resp.status);
            for (key, value) in resp.headers.iter() {
                let key_str = key.as_str();
                if !["transfer-encoding", "connection"].contains(&key_str) {
                    builder = builder.header(key_str, value.as_bytes());
                }
            }
            builder
                .body(Body::from(resp.body))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Err(ResilientError::CircuitOpen) => (
            StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(serde_json::json!({
                "error": "circuit_open",
                "service": service_name
            })),
        )
            .into_response(),
        Err(ResilientError::Timeout) => {
            tracing::warn!("forward to {} timed out", service_name);
            (StatusCode::GATEWAY_TIMEOUT, "upstream timeout").into_response()
        }
        Err(ResilientError::Connect(e)) => {
            tracing::warn!("forward to {} connect error: {:?}", service_name, e);
            (StatusCode::BAD_GATEWAY, "upstream error").into_response()
        }
    }
}
```

- [ ] **Step 3: 修改 main.rs 注入 egress_map 并移除 ServiceDiscovery 构造**

在 `services/api-gateway/src/main.rs`：

a. 修改 `use api_gateway::{...}` 移除 `ServiceDiscovery`（约 line 19）：
```rust
use api_gateway::{
    RouteConfig, RouteTable, RouterConfig,
    /* ServiceDiscovery 删除 */
};
```

b. 删除整个 `let discovery = Arc::new(ServiceDiscovery::new(&cfg.consul_addr));` 行（约 line 167）。

c. 将 `RouterConfig { discovery: discovery.clone(), resilient }` 替换为：
```rust
    let router_cfg = Arc::new(RouterConfig {
        resilient,
        egress_map: Arc::new(cfg.egress_map.clone()),
    });
```

> `cfg.consul_addr` 字段保留（用于其他子系统如 perm_epoch consumer 注册——若 grep 无其他引用，本 task 不动它，留给后续 cleanup task）。

- [ ] **Step 4: 修改 lib.rs 移除 discovery 模块**

将 `services/api-gateway/src/lib.rs`：
```rust
pub mod discovery;
pub use discovery::{ServiceDiscovery, ServiceInstance};
```
两段删除。

- [ ] **Step 5: 删除 discovery.rs**

```bash
git rm services/api-gateway/src/discovery.rs
```

- [ ] **Step 6: 编译 + 现有单测**

```bash
cargo build -p api-gateway
cargo test -p api-gateway --lib
```

Expected: 编译通过，现有单测通过。若 `tests/` 下有 integration test 引用了 `ServiceDiscovery`，先 grep 排查：
```bash
grep -rn "ServiceDiscovery\|discovery::" services/api-gateway/tests/
```
有命中则一并修复（删除断言或改用 `RouterConfig { resilient, egress_map }` 直构）。

- [ ] **Step 7: 添加 router 单测覆盖 egress_map 缺失场景**

在 `services/api-gateway/src/router.rs` 文件末尾追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use digua_common_http::ResilienceConfig;

    fn mk_cfg(map: HashMap<String, EgressTarget>) -> Arc<RouterConfig> {
        Arc::new(RouterConfig {
            resilient: Arc::new(ResilientClient::new(
                reqwest::Client::new(),
                ResilienceConfig::default(),
                Arc::new(digua_common_http::NoopObserver),
            )),
            egress_map: Arc::new(map),
        })
    }

    #[tokio::test]
    async fn unknown_service_returns_bad_gateway() {
        let cfg = mk_cfg(HashMap::new());
        let req = Request::builder().uri("/x").body(Body::empty()).unwrap();
        let resp = forward_to_service(cfg, req, "no-such-service", "/x").await;
        assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
    }
}
```

> 删除 discovery 后此测试可以**确定地**断言 `BAD_GATEWAY`（不再需要"server_error 二选一"妥协）—— router 在 egress_map miss 时就是返 BAD_GATEWAY，没有其他分支干扰。

- [ ] **Step 8: 跑单测**

```bash
cargo test -p api-gateway --lib router::tests
```

Expected: 测试通过，状态码精确等于 502。

- [ ] **Step 9: Commit**

```bash
git add services/api-gateway/
git commit -m "P5-A: gateway forwards via per-service egress map; drop ServiceDiscovery (sidecar EDS replaces)"
```

---

## Task 6: 在 compose 中声明 sidecar yaml anchors（DRY 基础）

**Files:**
- Modify: `infrastructure/docker-compose-services.yml`

**Interfaces:**
- Consumes: 无运行时依赖。
- Produces: 顶层 `x-sidecar-defaults` / `x-sidecar-env` anchor 可被后续 6 个 sidecar 服务引用。运行时无副作用（compose 忽略 `x-` 前缀的顶层键）。

> **决策**：6 个 sidecar 服务 12 行 yaml 几乎逐字重复（仅 `DP_PROXY_SERVICE_ID` + `ports` 不同）。用 docker-compose 原生 YAML anchor 抽公共部分，后续 Task 7-12 每个 sidecar 只 4 行 yaml。

- [ ] **Step 1: 在 services.yml 顶部插入 anchors**

将 `infrastructure/docker-compose-services.yml` **首行**（在 `services:` 之前）插入：

```yaml
# === Sidecar shared definitions (Phase 5-A mTLS) ===
# `x-` 前缀的顶层键被 compose spec 忽略，仅用于 YAML anchor 复用。
x-sidecar-defaults: &sidecar-defaults
  image: hashicorp/consul-dataplane:1.2
  mem_limit: 64m
  depends_on:
    consul:
      condition: service_healthy
  networks:
    - superpowers

x-sidecar-env: &sidecar-env
  DP_CONSUL_ADDRESSES: consul
  DP_CONSUL_GRPC_PORT: "8502"
  DP_ENVOY_READY_BIND_ADDR: 0.0.0.0
  DP_ENVOY_READY_BIND_PORT: "20100"
  DP_ENVOY_CONCURRENCY: "1"

services:
  api-gateway:
    # ...（原有内容保持不变，仅在文件顶部插入了 anchors 与新 services: 起始行）
```

> 注意：原文件已有 `services:` 顶层键，**只插入 anchor 块**，不要重复 `services:` 键。如果原文件 line 1 是 `services:`，则在它之前插入 anchor 块。

- [ ] **Step 2: 验证 compose 语法**

```bash
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml config > /dev/null
```

Expected: 退出码 0，无报错。compose 会展开 anchor 但本 task 还没引用，输出与原始等价。

- [ ] **Step 3: Commit**

```bash
git add infrastructure/docker-compose-services.yml
git commit -m "P5-A: add YAML anchors for sidecar defaults (DRY base for Task 7-12)"
```

---

## Task 7: iam-service 接入 sidecar

**Files:**
- Modify: `infrastructure/docker-compose-services.yml`

**Interfaces:**
- Consumes: Task 1, Task 3。
- Produces: `iam-sidecar` 容器存在并健康；`iam-service` 容器复用 sidecar 网络命名空间。

- [ ] **Step 1: 修改 compose 文件**

定位 `infrastructure/docker-compose-services.yml` 中 `iam-service:` 段（约 line 85-114）。

第一步：删除 `iam-service` 块下的 `ports:` 与 `networks:` 段；添加 `network_mode`；调整 depends_on：

```yaml
  iam-service:
    build:
      context: ..
      dockerfile: services/iam-service/Dockerfile
    network_mode: service:iam-sidecar
    volumes:
      - ../docker-example/secrets:/app/keys:ro
      - ../services/iam-service/config:/etc/iam-service:ro
    environment:
      SERVER_HOST: "127.0.0.1"
      SERVER_PORT: "8080"
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

第二步：在 `services:` 下追加 sidecar 定义（保持文件原 sidecar 段紧邻业务段，便于阅读）：

```yaml
  iam-sidecar:
    <<: *sidecar-defaults
    environment:
      <<: *sidecar-env
      DP_PROXY_SERVICE_ID: iam-service-sidecar-proxy
    ports: ["6002:20000"]
```

- [ ] **Step 2: 启动验证**

```bash
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml up -d iam-sidecar iam-service
sleep 12
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml ps iam-sidecar iam-service
```

Expected: 两个容器 `Up`，iam-sidecar 健康（envoy ready）。

```bash
# Sidecar 转发 mTLS-terminated 流量到业务进程；从外网络无法直连业务端口，只能走 sidecar ingress（端口 6002）。
# 验证 envoy ready：
curl -fsS http://localhost:20100/ready || echo "envoy not exposed externally (expected if 20100 not mapped)"
# 验证业务联通（gateway 走 mTLS 进 sidecar，外部测试只能走 host 6002）：
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml logs --tail=30 iam-sidecar
```

Expected: sidecar 日志显示 envoy listener 已绑定到 0.0.0.0:20000，无连续错误。

- [ ] **Step 3: Commit**

```bash
git add infrastructure/docker-compose-services.yml
git commit -m "P5-A: attach consul-dataplane sidecar to iam-service"
```

---

## Task 8: token-service 接入 sidecar

**Files:**
- Modify: `infrastructure/docker-compose-services.yml`

**Interfaces:**
- Consumes: Task 1, Task 3。
- Produces: `token-sidecar` 容器存在，`token-service` 复用其网络命名空间。

- [ ] **Step 1: 修改 token-service 块**

将 `infrastructure/docker-compose-services.yml` 中 `token-service:` 段（约 line 46-83）改为：

```yaml
  token-service:
    build:
      context: ..
      dockerfile: services/token-service/Dockerfile
    network_mode: service:token-sidecar
    volumes:
      - ../docker-example/secrets:/app/keys
      - ../docker-example/config/token-service:/app/config/token-service
      - ../services/token-service/config:/app/config
    environment:
      SERVER_HOST: "127.0.0.1"
      SERVER_PORT: "8080"
      REDIS_URL: "redis://redis:6379"
      DATABASE_HOST: mysql
      DATABASE_PORT: "3306"
      DATABASE_USER: root
      DATABASE_PASSWORD: rootpass
      DATABASE_NAME: superpowers
      TS_JWT_PRIVATE_KEY_PATH: "/app/keys/private.pem"
      TS_JWT_PUBLIC_KEY_PATH: "/app/keys/public.pem"
      TS_CLIENTS_FILE_PATH: "/app/config/clients.json"
      TS_CONSUL_ADDR: "http://consul:8500"
      CONFIG_FILE: "/config/config.yaml"
      OTEL_EXPORTER_OTLP_ENDPOINT: "http://otel-collector:4317"
    healthcheck:
      test: ["CMD", "busybox", "wget", "-q", "--spider", "http://127.0.0.1:8080/.well-known/jwks.json"]
      interval: 10s
      timeout: 5s
      retries: 5
    depends_on:
      redis:
        condition: service_healthy
      mysql:
        condition: service_healthy
      token-sidecar:
        condition: service_started
```

> 注意：`network_mode: service:X` 下 healthcheck 在业务容器内执行，访问 `127.0.0.1:8080`（本地回环，命名空间共享）。

- [ ] **Step 2: 追加 token-sidecar 定义**

在 services 段中追加：

```yaml
  token-sidecar:
    <<: *sidecar-defaults
    environment:
      <<: *sidecar-env
      DP_PROXY_SERVICE_ID: token-service-sidecar-proxy
    ports: ["6001:20000"]
```

- [ ] **Step 3: 验证**

```bash
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml up -d token-sidecar token-service
sleep 12
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml ps token-sidecar token-service
```

Expected: 都 `Up`，token-service 健康检查通过。

- [ ] **Step 4: Commit**

```bash
git add infrastructure/docker-compose-services.yml
git commit -m "P5-A: attach consul-dataplane sidecar to token-service"
```

---

## Task 9: notification-service 接入 sidecar

**Files:**
- Modify: `infrastructure/docker-compose-services.yml`
- Modify: `docker-example/config/notification-service/config.yaml`

**Interfaces:**
- Consumes: Task 1, Task 4（运行时注册 sidecar_service）。
- Produces: `notification-sidecar` 容器；notification-service 进程监听 `127.0.0.1:8080`（在 sidecar netns 内回环），mTLS bypass 不可达。

> **安全约束（4A）**：notification-service 用 yaml 配置文件读 `server.http_addr`（不响应 `SERVER_HOST` env），必须直接改 yaml 把绑定地址改成 `127.0.0.1:8080`。否则同网络容器可绕过 sidecar 直连业务端口。

- [ ] **Step 1: 改 notification-service 配置文件绑回环**

将 `docker-example/config/notification-service/config.yaml` 顶部：

```yaml
server:
  http_addr: "0.0.0.0:8080"
```

改为：

```yaml
server:
  http_addr: "127.0.0.1:8080"
```

> 配置文件改动后业务进程监听仅本机回环，外部容器只能通过 sidecar:20000 mTLS ingress 访问。

- [ ] **Step 2: 修改 notification-service 块**

将 `notification-service:` 段（约 line 158-188）改为：

```yaml
  notification-service:
    build:
      context: ..
      dockerfile: services/notification-service/Dockerfile
    network_mode: service:notification-sidecar
    environment:
      EVENT_BUS_URL: "amqp://guest:guest@rabbitmq:5672/%2f"
      CONSUL_ADDR: "http://consul:8500"
      SERVER_PORT: "8080"
      DATABASE_HOST: mysql
      DATABASE_PORT: "3306"
      DATABASE_USER: root
      DATABASE_PASSWORD: rootpass
      DATABASE_NAME: superpowers
      REDIS_URL: "redis://redis:6379"
      OTEL_EXPORTER_OTLP_ENDPOINT: "http://otel-collector:4317"
    depends_on:
      mysql:
        condition: service_healthy
      redis:
        condition: service_healthy
      consul:
        condition: service_healthy
      rabbitmq:
        condition: service_healthy
      notification-sidecar:
        condition: service_started
    volumes:
      - ../docker-example/config/notification-service:/app/config:ro
```

- [ ] **Step 3: 追加 sidecar 定义**

```yaml
  notification-sidecar:
    <<: *sidecar-defaults
    environment:
      <<: *sidecar-env
      DP_PROXY_SERVICE_ID: notification-service-sidecar-proxy
    ports: ["6003:20000"]
```

- [ ] **Step 4: 验证 + Commit**

```bash
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml up -d notification-sidecar notification-service
sleep 12
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml logs --tail=20 notification-service notification-sidecar
# 验证业务进程绑回环（关键安全约束 4A）
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml exec api-gateway wget -q -O- --timeout=3 http://notification-sidecar:8080/health 2>&1 | head -3 || echo "bypass denied as expected"
git add infrastructure/docker-compose-services.yml docker-example/config/notification-service/config.yaml
git commit -m "P5-A: attach consul-dataplane sidecar to notification-service (loopback bind)"
```

Expected: 启动日志显示业务 listening on 127.0.0.1:8080 + Consul 注册成功（Connect 字段已生效）；从 api-gateway 容器直连 notification-sidecar:8080 失败（bypass denied）。

---

## Task 10: audit-service 接入 sidecar

**Files:**
- Modify: `infrastructure/docker-compose-services.yml`

**Interfaces:**
- Consumes: Task 1, Task 4。

- [ ] **Step 1: 修改 audit-service 块**

将 `audit-service:` 段（约 line 214-243）改为：

```yaml
  audit-service:
    build:
      context: ..
      dockerfile: services/audit-service/Dockerfile
    network_mode: service:audit-sidecar
    environment:
      SERVER_HOST: "127.0.0.1"
      SERVER_PORT: "8080"
      DATABASE_HOST: mysql
      DATABASE_PORT: "3306"
      DATABASE_USER: root
      DATABASE_PASSWORD: rootpass
      DATABASE_NAME: superpowers
      CONSUL_ADDR: "http://consul:8500"
      CONSUL_SERVICE_NAME: "audit-service"
      EVENT_BUS_URL: "amqp://guest:guest@rabbitmq:5672/%2f"
      REDIS_URL: "redis://redis:6379"
      RUST_LOG: "info"
    depends_on:
      mysql:
        condition: service_healthy
      redis:
        condition: service_healthy
      consul:
        condition: service_healthy
      rabbitmq:
        condition: service_healthy
      audit-sidecar:
        condition: service_started
```

- [ ] **Step 2: 追加 sidecar 定义**

```yaml
  audit-sidecar:
    <<: *sidecar-defaults
    environment:
      <<: *sidecar-env
      DP_PROXY_SERVICE_ID: audit-service-sidecar-proxy
    ports: ["6005:20000"]
```

- [ ] **Step 3: 验证 + Commit**

```bash
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml up -d audit-sidecar audit-service
sleep 12
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml ps audit-sidecar audit-service
git add infrastructure/docker-compose-services.yml
git commit -m "P5-A: attach consul-dataplane sidecar to audit-service"
```

---

## Task 11: prue_backend 接入 sidecar

**Files:**
- Modify: `infrastructure/docker-compose-services.yml`

**Interfaces:**
- Consumes: Task 1, Task 3。

- [ ] **Step 1: 修改 prue_backend 块**

将 `prue_backend:` 段（约 line 139-156）改为：

```yaml
  prue_backend:
    build:
      context: ..
      dockerfile: services/prue_backend/Dockerfile
    network_mode: service:prue-sidecar
    environment:
      SERVER_HOST: "127.0.0.1"
      SERVER_PORT: "8080"
      DATABASE_URL: "mysql://root:rootpass@mysql:3306/superpowers"
      RUST_LOG: "info"
    depends_on:
      mysql:
        condition: service_healthy
      consul:
        condition: service_healthy
      prue-sidecar:
        condition: service_started
```

> Sidecar 容器名使用连字符 `prue-sidecar`（容器名规约），但 Consul service id / proxy id 仍是 `prue_backend-sidecar-proxy`（匹配服务名）。

- [ ] **Step 2: 追加 sidecar 定义**

```yaml
  prue-sidecar:
    <<: *sidecar-defaults
    environment:
      <<: *sidecar-env
      DP_PROXY_SERVICE_ID: prue_backend-sidecar-proxy
    ports: ["6004:20000"]
```

- [ ] **Step 3: 验证 + Commit**

```bash
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml up -d prue-sidecar prue_backend
sleep 12
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml ps prue-sidecar prue_backend
git add infrastructure/docker-compose-services.yml
git commit -m "P5-A: attach consul-dataplane sidecar to prue_backend"
```

---

## Task 12: api-gateway 接入 sidecar（含 5 个 egress upstreams）

**Files:**
- Modify: `infrastructure/docker-compose-services.yml`

**Interfaces:**
- Consumes: Task 3（gateway 在 Consul 已声明 5 个 upstream `local_bind_port`），Task 5（gateway 代码用 egress_map）。
- Produces: `gateway-sidecar` 容器同时暴露入口 3000 与所有 egress 端口；gateway 业务容器通过 `127.0.0.1:18001..18005` 走 mTLS。

- [ ] **Step 1: 修改 api-gateway 块**

将 `api-gateway:` 段（约 line 2-44）改为：

```yaml
  api-gateway:
    image: zhcoder-docker-registry.com:8000/micro-server/api-gateway:latest
    network_mode: service:gateway-sidecar
    environment:
      RUST_LOG: "api_gateway=trace,info"
      GW_SERVER__HOST: "0.0.0.0"
      GW_SERVER__PORT: "3000"
      GW_REDIS__URL: "redis://redis:6379"
      GW_REDIS__PREFIX: "superpowers:"
      GW_CONSUL_ADDR: "http://consul:8500"
      GW_JWKS_URL: "http://127.0.0.1:18002/.well-known/jwks.json"
      GW_ISSUER: "superpowers-token-service"
      GW_ROUTES_DIR: "/app/config/api-gateway/routes"
      GW_JWKS_PUBLIC_KEY_PATH: "/app/keys/public.pem"
      GW_EVENT_BUS_URL: "amqp://guest:guest@rabbitmq:5672/%2f"
      OTEL_EXPORTER_OTLP_ENDPOINT: "http://otel-collector:4317"
    entrypoint: ["sh", "-c"]
    command:
      - |
        i=0
        until wget -q --spider http://127.0.0.1:18002/.well-known/jwks.json; do
          i=$$((i+1))
          if [ $$i -ge 60 ]; then echo "token-service jwks not ready, giving up"; exit 1; fi
          echo "waiting token-service jwks via sidecar... ($$i)"
          sleep 1
        done
        exec /app/api-gateway serve --routes /app/config/api-gateway/routes
    volumes:
      - ./config/api-gateway/routes:/app/config/api-gateway/routes
      - ../docker-example/secrets:/app/keys:ro
    depends_on:
      consul:
        condition: service_healthy
      redis:
        condition: service_healthy
      rabbitmq:
        condition: service_healthy
      gateway-sidecar:
        condition: service_started
```

> JWKS 端点从 `http://token-service:8080/.well-known/jwks.json` 改为 `http://127.0.0.1:18002/.well-known/jwks.json` —— gateway 进程只通过自己的 sidecar egress 访问下游。

- [ ] **Step 2: 追加 gateway-sidecar 定义（egress-only）**

> Gateway 是系统边界，对外 HTTP+用户 JWT，**不需要** ingress mTLS。所以 sidecar：
> - **不**声明 `:20000` ingress 映射
> - 声明 `"3000:3000"` 把业务进程绑定的 3000 暴露给 host（因为 `network_mode: service:gateway-sidecar` 让 sidecar 容器持有 netns，业务容器不能自己开 `ports:`）
> - Egress 端口 18001-18005 是 sidecar 内部回环 listener，无需 `ports:` 映射，gateway 进程通过共享 netns 访问。

```yaml
  gateway-sidecar:
    <<: *sidecar-defaults
    environment:
      <<: *sidecar-env
      DP_PROXY_SERVICE_ID: api-gateway-sidecar-proxy
    ports: ["3000:3000"]
```

**对应地**，Task 3 的 Consul 注册中 api-gateway 的 sidecar_service 必须**关闭 ingress listener**。在 `docker-example/config/consul/config.docker.json` 的 api-gateway 段，`connect.sidecar_service.proxy` 已只声明 `upstreams[]`（没有 `local_service_port` 触发的 ingress），保持现状即可——Connect 默认 ingress 行为可通过 proxy 配置裁剪；envoy ingress listener 在没有 ingress flag 时仍可能默认创建。**实施验证**：启动后从外部 host 访问 sidecar 容器的 :20000（如果 docker port 没映射就不可达，这是预期）；如担心仍有 ingress listener 占内存，加 `proxy.config: { protocol: tcp }` 或显式不声明 `local_service_port` —— 当前 services.json 的 gateway 段未设置 local_service_port（继承 Service.Port=3000），但因为没有 `:20000` 端口映射给 host，外部无法接触 ingress listener，安全性等价。

- [ ] **Step 3: 重建并验证**

```bash
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml up -d --build api-gateway gateway-sidecar
sleep 15
# 外部明文直达业务进程
curl -fsS http://localhost:3000/health
# Gateway 通过 sidecar egress 走 mTLS 访问下游
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml exec api-gateway wget -q -O- http://127.0.0.1:18002/.well-known/jwks.json | head -c 200
# Sidecar 不应监听 :20000（gateway 边界，没有 ingress mTLS）
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml exec gateway-sidecar sh -c 'wget -q -O- --timeout=2 http://127.0.0.1:20000/ 2>&1 || echo "no ingress as expected"'
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml logs --tail=30 gateway-sidecar | grep -i "listener\|cluster"
```

Expected: `/health` 返 200；jwks 通过 egress 拿到；sidecar :20000 无监听（"no ingress as expected"）；日志显示 5 个 outbound cluster 已配。

- [ ] **Step 4: 验证 docker 内部 DNS 解析（frontend 网络可达性）**

```bash
# 从 frontend 容器（或任何已加入 superpowers 网络的容器）验证 api-gateway 服务名仍可达
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml up -d frontend
sleep 8
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml exec frontend sh -c 'getent hosts api-gateway && wget -q -O- --timeout=5 http://api-gateway:3000/health'
```

Expected: 解析返回非空 IP（应为 `gateway-sidecar` 容器 IP），`/health` 返回 200。

> 原理：`network_mode: service:gateway-sidecar` 让 `api-gateway` 业务容器**无独立网络栈**，docker DNS 对 "api-gateway" 服务名的解析会返回 sidecar 容器 IP；业务进程在 sidecar netns 内绑定 :3000，外部访问可达。这是预期行为。

如解析失败（getent 返空 / wget 连接被拒），停下排查——可能是 docker 版本对 `network_mode: service` 的 DNS 行为差异；fallback 方案是在 `gateway-sidecar` services 段加 `networks.superpowers.aliases: [api-gateway]` 显式注入 alias。

- [ ] **Step 5: Commit**

```bash
git add infrastructure/docker-compose-services.yml
git commit -m "P5-A: attach gateway sidecar with 5 mTLS egress upstreams (no ingress)"
```

- [ ] **Step 6: 打 pre-mtls 回滚锚点 tag**

```bash
git tag pre-mtls HEAD
echo "tagged pre-mtls at $(git rev-parse HEAD) — sidecar fully deployed, require_service_token still present"
```

> Tag 与它指向的 commit 在同一 task 内完成（不依赖未来 grep 匹配 commit message）。此 tag 是"sidecar 已就位但 service token 中间件还在"的回滚锚点 —— 紧急回退时可 `git reset --hard pre-mtls` 整体退回到本 task 末态，或保留 mTLS 仅 `git revert <Task 13 commit>` 单独恢复 service token 中间件。

---

## Task 13: 删除 require_service_token

**Files:**
- Modify: `libs/digua_authz/src/lib.rs`
- Delete: `libs/digua_authz/src/require_service_token.rs`
- Delete: `libs/digua_authz/tests/require_service_token_test.rs`

**Interfaces:**
- Consumes: 无运行时依赖（Task 12 后 mTLS 已生效）。
- Produces: `cargo build --workspace` 通过；`grep -rn require_service_token --include='*.rs'` 仅在历史 plan/spec 文档中匹配。

- [ ] **Step 1: 再次确认无业务代码调用**

```bash
grep -rn "require_service_token" --include='*.rs' .
```

Expected: 仅 `libs/digua_authz/src/lib.rs` / `libs/digua_authz/src/require_service_token.rs` / `libs/digua_authz/tests/require_service_token_test.rs` 三处。若 services/ 下有命中，停下报告（与本计划假设不符）。

- [ ] **Step 2: 修改 lib.rs**

将 `libs/digua_authz/src/lib.rs` 整文件替换为：

```rust
pub mod require_role;
pub mod perms;

pub use require_role::require_role;
```

- [ ] **Step 3: 删除文件**

```bash
git rm libs/digua_authz/src/require_service_token.rs libs/digua_authz/tests/require_service_token_test.rs
```

- [ ] **Step 4: 编译验证**

```bash
cargo build --workspace
cargo test -p digua_authz
```

Expected: 全部通过。

- [ ] **Step 5: Commit**

```bash
git add libs/digua_authz/src/lib.rs
git commit -m "P5-A: remove require_service_token middleware (mTLS replaces service identity)"
```

---

## Task 14: 编写运维文档

**Files:**
- Create: `docs-site/guides/mtls-deployment.md`

- [ ] **Step 1: 写入文档**

`docs-site/guides/mtls-deployment.md`：

```markdown
# mTLS 服务网格部署与运维

> Phase 5-A 引入。基于 Consul Connect + consul-dataplane 1.2。

## 1. 启动顺序

\`\`\`bash
docker compose -f infrastructure/docker-compose-infra.yml \
               -f infrastructure/docker-compose-services.yml up -d
\`\`\`

Compose 已声明 `depends_on`：consul 优先，sidecar 等 consul healthy，业务容器等 sidecar started。

## 2. CA 根证书

\`\`\`bash
# 备份根证书 PEM
curl -fsS http://localhost:8500/v1/connect/ca/roots | jq -r '.Roots[0].RootCertPEM' > ./docker-example/secrets/consul-ca-root.pem
\`\`\`

dev 模式下 Consul 重启会重新生成 CA，所有 sidecar 自动重新签发；生产环境需挂持久化 data dir。

## 3. 证书轮换

默认叶证书 72h 有效期，到期前 60min 由 consul-dataplane 自动向 Consul 申请新证书并热替换。
人工触发轮换：

\`\`\`bash
docker compose restart <svc>-sidecar
\`\`\`

## 4. 端口规约

见 `infrastructure/sidecar/README.md`。

- 业务容器**只**与本机 sidecar 同 netns，**不**直接监听 host 端口。
- Gateway 的 egress 端口 `18001..18005` 只在 sidecar 容器内回环监听，gateway 业务进程访问 `127.0.0.1:1800X`。
- Gateway 边界例外：gateway-sidecar **不开 ingress mTLS**（业务进程直接监听 :3000，由 sidecar 容器暴露 `"3000:3000"` 给 host）。

## 4.1 网络拓扑（mTLS 后）

\`\`\`
                            host:3000
                                │
                                ▼
   ┌────────────────────────────────────────────┐
   │  gateway-sidecar (持有 netns)             │
   │   ├─ :3000 ── (本地端口)── api-gateway 进程│  ◄── 外部明文（user JWT）
   │   ├─ :18001 ── envoy egress ──┐           │
   │   ├─ :18002 ── envoy egress ──┤           │
   │   ├─ :18003 ── envoy egress ──┤  mTLS out │
   │   ├─ :18004 ── envoy egress ──┤           │
   │   └─ :18005 ── envoy egress ──┘           │
   └──────────────────┬─────────────────────────┘
                      │ mTLS (Consul CA-signed)
                      ▼
   ┌──────────────────────────────────────┐
   │  iam-sidecar (持有 netns)            │
   │   ├─ :20000 ── envoy ingress ◄── mTLS terminate
   │   │                │
   │   │                ▼
   │   └─ :8080 ── iam-service 进程       │
   └──────────────────────────────────────┘
                      ▲
                      │ (重复 6 个下游 sidecar)
   token-sidecar / notification-sidecar / audit-sidecar / prue-sidecar / ...

   docker DNS 行为：业务服务名（如 "api-gateway"）解析到 sidecar 容器 IP，
                    因为业务容器走 network_mode: service 没有独立网络栈。
\`\`\`

## 5. 故障排查

| 现象 | 排查命令 |
|------|----------|
| Sidecar 启动失败 | `docker compose logs <svc>-sidecar` 看 envoy bootstrap 错误 |
| envoy 无 cluster | `curl http://localhost:20100/clusters`（在容器内 exec） |
| mTLS 握手失败 | sidecar 日志搜 `TLS error`；确认 destination service name 在 Consul 已注册并健康 |
| gateway 502 | gateway 日志搜 `connect error`；确认对应 egress_map 端口存在 |
| Consul Connect 未启用 | `curl http://localhost:8500/v1/connect/ca/roots`，空数组即未启用 |
| **同网络容器绕过 sidecar 直连业务端口** | 业务进程绑了 `0.0.0.0` 而非 `127.0.0.1` —— 检查每个业务服务的 `SERVER_HOST` env / `http_addr` 配置，必须监听 sidecar netns 内回环（关键安全约束 4A）|

## 6. 回滚

回滚锚点：tag `pre-mtls` 打在 Task 12 commit（sidecar 全部部署完，`require_service_token` 尚未删除）。

| 场景 | 操作 |
|------|------|
| 只想保留 mTLS、恢复 service token 中间件 | `git revert <Task 13 commit hash>`（重新引入 `require_service_token`） |
| 整体退回 mTLS 之前 | `git reset --hard pre-mtls` 然后 `git revert HEAD..pre-mtls` 倒着撤所有 Task 1-12 改动；或直接基于 `pre-mtls` 之前的 commit 重建分支 |
| 临时绕过 mTLS（不改代码） | 在 `docker-compose-services.yml` 中把单个业务服务的 `network_mode: service:<svc>-sidecar` 改回 `networks: [superpowers]` + 恢复原 `ports:` 段，停掉对应 sidecar 容器 —— 流量降级为明文，但 gateway egress_map 仍指本机 sidecar 端口（连接失败 → 熔断 → 502），所以**必须**同时调整 gateway egress_map 临时指向 docker 服务名 |

## 7. 关键安全约束（必读）

**业务进程必须监听 sidecar netns 内回环地址（`127.0.0.1`），不能监听 `0.0.0.0`。**

否则同 docker 网络的其他容器（如 frontend、其他业务服务）可直连 `<svc>-sidecar:8080` **绕过 sidecar mTLS**，整个部署失去身份认证保护。

| 服务 | 配置入口 | 正确值 |
|------|----------|--------|
| iam-service | env `SERVER_HOST` | `127.0.0.1` |
| token-service | env `SERVER_HOST` | `127.0.0.1` |
| audit-service | env `SERVER_HOST` | `127.0.0.1` |
| prue_backend | env `SERVER_HOST` | `127.0.0.1` |
| notification-service | yaml `server.http_addr` | `127.0.0.1:8080` |
| api-gateway | env `GW_SERVER__HOST` | `0.0.0.0` （边界例外）|

Task 15 Step 2 的 3 条断言会校验此约束；上线前**必须**确认 3 条都通过。

## 8. 已知限制

- `iam-outbox-publisher` / `notification-outbox-publisher` 不接 sidecar（无 HTTP listener）。
- `config-service` / `file-service` 源码存在但未部署，启用时需按 `infrastructure/sidecar/README.md` 模板补端口规约。
- 测试容器（testcontainers）跑单测时不经 sidecar，仍走明文——单测只测业务逻辑，不验 mTLS。
```

- [ ] **Step 2: Commit**

```bash
git add docs-site/guides/mtls-deployment.md
git commit -m "P5-A: document mTLS deployment, rotation, and rollback procedure"
```

---

## Task 15: 端到端回归 + TODO 标记

**Files:**
- Modify: `TODO.md`

**Interfaces:**
- Consumes: Task 1-14 全部完成。
- Produces: `node scripts/run-e2e-local.js` 全量通过；TODO Phase 5-A 段勾选。

- [ ] **Step 1: 重建全量栈**

```bash
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml down
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml up -d --build
sleep 30
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml ps
```

Expected: 全部 `Up` / `healthy`。

- [ ] **Step 2: 验证 mTLS 强制（3 条断言）**

```bash
# 选用 frontend 容器（普通业务容器，未持 Consul CA 客户端证书）做攻击者视角

# 正向：通过 gateway egress mTLS 访问下游（应通）
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml exec api-gateway sh -c '
  echo "[正向] mTLS via gateway egress:";
  wget -q -O- --timeout=5 http://127.0.0.1:18001/health && echo " -> OK" || echo " -> FAIL (应 OK)"
'

# 负向 A：从同网络容器直连业务端口（应失败 —— 业务进程绑 127.0.0.1，对外不可达）
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml exec frontend sh -c '
  echo "[负向 A] 业务端口直连 bypass:";
  wget -q -O- --timeout=3 http://iam-sidecar:8080/health 2>&1 && echo " -> LEAK!!! 业务进程仍绑 0.0.0.0，mTLS bypass 可达" || echo " -> denied as expected (业务进程仅监听 127.0.0.1)"
'

# 负向 B：从同网络容器明文访问 sidecar ingress（应失败 —— envoy 强制 mTLS）
docker compose -f infrastructure/docker-compose-infra.yml -f infrastructure/docker-compose-services.yml exec frontend sh -c '
  echo "[负向 B] sidecar ingress 明文访问:";
  wget -q -O- --timeout=3 http://iam-sidecar:20000/health 2>&1 && echo " -> LEAK!!! sidecar 未强制 mTLS" || echo " -> denied as expected (envoy TLS handshake required)"
'
```

Expected:
- `[正向] -> OK`
- `[负向 A] -> denied as expected`（如出现 `LEAK!!!` 说明业务进程仍绑 0.0.0.0，**立即停下检查 Task 7-11 的 SERVER_HOST 是否改成 127.0.0.1**）
- `[负向 B] -> denied as expected`

任意一条 `LEAK!!!` 出现 → mTLS 部署有安全漏洞，**不要进入下一步**。

- [ ] **Step 3: E2E 回归**

```bash
node scripts/run-e2e-local.js
```

Expected: 全量通过。如失败，关注 gateway 日志中 `connect error` / `circuit_open` 与 sidecar 日志中 envoy cluster 状态。

- [ ] **Step 4: 更新 TODO**

打开 `TODO.md`，定位 Phase 5-A 段（约 line 95-100），将：

```markdown
  - [ ] **A — mTLS / 服务网格**
    - [ ] Consul Connect Dataplane Sidecar 集成
    - [ ] 淘汰应用层 `require_service_token` 方案
```

改为：

```markdown
  - [x] **A — mTLS / 服务网格**
    - [x] Consul Connect Dataplane Sidecar 集成
    - [x] 淘汰应用层 `require_service_token` 方案
```

- [ ] **Step 5: Commit**

```bash
git add TODO.md
git commit -m "P5-A: mark Phase 5-A complete after e2e regression"
```

> `pre-mtls` 回滚 tag 已在 Task 12 Step 6 打过，本 task 不重复处理。

---

## 自审 (Self-Review)

**1. Spec 覆盖：**

| Spec 章节 | 覆盖 Task |
|-----------|-----------|
| §4.1 服务发现变更（`connect.sidecar_service`） | Task 3, 4 |
| §4.2 网关转发改 sidecar egress | Task 5 |
| §4.3 删除 `require_service_token` | Task 13 |
| §5.1 Consul Connect CA | Task 1 |
| §5.2 业务服务 + Sidecar 模板 | Task 2, 6, 7-11 |
| §5.3 api-gateway sidecar | Task 12 |
| §6 证书生命周期 | Task 14 文档 |
| §7 风险与回滚 | Task 14 文档 |
| §8 验证方式 | Task 15 |

未直接出现在 spec 但对落地必需的补充：
- Spec §5.2 的 sidecar egress port 描述模糊（"localhost:19001"），实际 consul-dataplane 必须**每上游一个 listener port**——本计划 Task 3 / Global Constraints 显式映射 18001-18005。
- Spec 提到 7 服务，但 `config-service`/`file-service` 当前不在 compose——本计划缩减为实际 5 + gateway，并在 Task 14 文档注明。

**2. 占位符扫描：** 无 "TBD" / "TODO" / "implement later" / "fill in"。所有"先 grep / 先定位"步骤都给出**精确的 expected 命中文件清单**（如 Task 4 Step 1、Task 5 Step 6），可作为确定性 sanity check 而非自由探索。

**3. 类型一致性：**
- `EgressTarget { host: String, port: u16 }` 在 Task 5 Step 1 定义；Task 5 Step 2 router.rs 通过 `egress.host` / `egress.port` 引用——匹配。
- `RouterConfig.egress_map` 字段类型 `Arc<HashMap<String, EgressTarget>>`——Task 5 三个 step 一致。
- Consul HTTP API 注册字段 PascalCase（`Connect.SidecarService.ID`）vs JSON config 文件 snake_case（`connect.sidecar_service.id`）—— Task 3（config.docker.json）用 snake_case，Task 4（`AgentServiceRegistration` 序列化）通过 `#[serde(rename_all = "PascalCase")]` + `#[serde(rename = "HTTP")]` 显式 PascalCase。两套都对，分别匹配 Consul agent 加载 config-dir 与 HTTP API 的两种约定。
- Sidecar 端口约定：ingress :20000, ready :20100, egress 18001-18005——`infrastructure/sidecar/README.md` 与各 task 一致。

---

## Execution Handoff

Plan complete. 两种执行选项：

1. **Subagent-Driven（推荐）** — 每个 task 派发新 subagent 执行 + review，迭代快。需使用 `superpowers:subagent-driven-development`。
2. **Inline Execution** — 在当前会话按批次执行，到 checkpoint 暂停 review。需使用 `superpowers:executing-plans`。

请选择执行方式。
