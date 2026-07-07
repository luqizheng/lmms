# Phase 5-A：Consul Connect 服务网格（mTLS + 意图 + 可观测性）— 设计

> ⚠️ **Superseded by** [`docs/superpowers/specs/2026-07-07-deprecate-consul-connect-use-registrator-design.md`](2026-07-07-deprecate-consul-connect-use-registrator-design.md) (2026-07-07). The Consul Connect / mTLS / sidecar approach described here was rolled back in favor of direct HTTP + JWT passthrough and `gliderlabs/registrator` for service discovery. Retained for historical reference only; do not follow this document for new work.

**日期**：2026-07-03 · 实施完成 2026-07-04 · **被 phase5a-2 弃用 2026-07-07**
**状态**：已被 phase5a-2 取代（弃用原因：内存成本 / 操作摩擦 / 替代方案足够）
**关联**：`TODO.md` Phase 5-A · `docs/superpowers/plans/2026-06-25-overall-roadmap.md` § Phase 5

---

## 背景

项目 7 个 Rust 业务服务（api-gateway / token-service / iam-service / notification-service / audit-service / config-service / file-service）+ prue_backend 适配服务，当前通过 docker-compose 部署在共享 `superpowers` 网络上。服务间 HTTP 通信为**明文**，仅靠应用层 service-token JWT（`aud: "service"`）做 `/iam/internal/*` 端点的服务身份校验。

Phase 5-A 目标：引入完整服务网格，在传输层实现 mTLS 加密 + 服务身份认证 + 授权策略（意图），并接入 mesh 级可观测性，淘汰应用层 `require_service_token` 方案。

**项目阶段约束**：开发阶段、从未发布、无遗留数据、无在线流量、不考虑向后兼容——因此采用一次性全量切换，无需渐进式 rollout 或并行过渡期。

**设计决策（brainstorm 2026-07-03）**：

- 部署拓扑：近期继续 docker-compose（K3s/K8s 不在近期计划）
- 范围：完整服务网格（mTLS + 身份 + 授权策略 + 可观测性），非仅 mTLS
- 内存：宽松，可接受 sidecar 50-80MB/服务
- 选型：Consul Connect（复用现有 Consul 1.15，支持非 K8s，K8s 迁移时意图模型可延续）

---

## 1. 架构与拓扑

### 1.1 Consul Connect 启用

`infrastructure/docker-compose-infra.yml` 已有 `consul:1.15`（dev 模式），`connect.hcl` 配置已注释。取消注释并启用 Connect，Consul 自带 Envoy，内置 CA 作为信任根。

### 1.2 Sidecar 模型 — 每服务一个 sidecar proxy

每个服务容器获得一个 Envoy sidecar（`consul connect envoy -sidecar-for <service-id>`）：

```
┌─────────────────────────────────────────────────┐
│  api-gateway 容器                               │
│  ┌──────────────┐    ┌──────────────────────┐   │
│  │ api-gateway  │───▶│ envoy sidecar (out)  │   │
│  │ 绑定 :3000   │    │ mTLS 出站到上游      │   │
│  └──────────────┘    └──────────────────────┘   │
└─────────────────────────────────────────────────┘
                          │ mTLS (Consul CA)
┌─────────────────────────┼─────────────────────┐
│  iam-service 容器       ▼                     │
│  ┌──────────────────────┐  ┌──────────────┐   │
│  │ envoy sidecar (in)   │─▶│ iam-service  │   │
│  │ :8080, mTLS 终止     │  │ 绑定 :18080  │   │
│  └──────────────────────┘  └──────────────┘   │
└───────────────────────────────────────────────┘
```

### 1.3 端口拆分

- **应用**绑定 `127.0.0.1:18080`（仅容器内可达，不再暴露 docker 网络）
- **Envoy sidecar**绑定网络端口 `:8080`（docker 网络可达），终止 mTLS，转发明文到本地应用

### 1.4 出站调用方改造

调用方（gateway / token / notification）的出站地址从 `http://iam-service:8080` 改为本地 sidecar proxy 端口（如 `http://127.0.0.1:<proxy-port>`，映射到 `iam-service`）。`api-gateway.toml` 的 `egress_map` 同步改。

### 1.5 待 mesh 覆盖的东西向 HTTP 调用

| 调用方 | 目标 | 调用方式 |
|--------|------|----------|
| api-gateway | iam / token / notification / audit / config / file / prue | `forward_to_service`（egress_map DNS） |
| token-service | iam-service | `fetch_json`（ResilientClient） |
| notification-service | iam-service | `/iam/internal/users/by-{role,dept}`（`iam_base_url`） |

RabbitMQ / MySQL / Redis 连接不在 mesh 范围内（基础设施层）。

### 1.6 实际落地：consul connect proxy（非 envoy）

原设计的 Envoy sidecar 方案因以下约束调整为 Consul 内置 TCP 代理：

1. **受限网络**：无法从外部下载 envoy 二进制（`envoyproxy/envoy` 镜像、GitHub release、Alpine apk 均不可达）
2. **Consul 1.15 的 `consul connect envoy -sidecar-for` bug**：`-sidecar-for` 无法通过 UUID 后缀的 service ID 定位 sidecar proxy
3. **端口冲突**：`network_mode: service:X` 下 envoy 和主服务若绑定同一端口会冲突

**最终方案：`consul connect proxy`（Consul 1.15 内置 Go TCP 代理）**

```
┌──────────────────────────────────────────────────────┐
│  api-gateway-sidecar 容器                             │
│  consul connect proxy -service api-gateway \          │
│    -listen ':3000' \              ← 入站 mTLS        │
│    -upstream iam-service:9081 \   ← 出站 mTLS        │
│    -upstream token-service:9082 \                    │
│    ... 7 个 upstream                                 │
│                                   │                   │
│                              network_mode: service    │
│  ┌────────────────────────────────────────────┐      │
│  │ api-gateway 容器                           │      │
│  │ 绑定 127.0.0.1:18000（内端口）              │      │
│  │ → upstream: 127.0.0.1:9081 (sidecar proxy) │      │
│  └────────────────────────────────────────────┘      │
└──────────────────────────────────────────────────────┘
                            │ mTLS (Consul CA)
┌───────────────────────────┼──────────────────────────┐
│  iam-service-sidecar 容器  ▼                          │
│  consul connect proxy -service iam-service \         │
│    -listen ':9081' \                           │      │
│    -service-addr 127.0.0.1:18081                      │
│                                                       │
│  ┌────────────────────────────────────────────┐      │
│  │ iam-service 容器                           │      │
│  │ 绑定 127.0.0.1:18081（内端口）              │      │
│  └────────────────────────────────────────────┘      │
└───────────────────────────────────────────────────────┘
```

**端口方案**（内外分离，无冲突）：
- **内端口**（180xx）：只能在本 network namespace 内访问，通过 `network_mode: service:X-sidecar` 共享
- **外端口**（908x / 3000）：sidecar 容器的 `ports:` 映射到 host，`consul connect proxy -listen` 在此执行 mTLS

**关键发现**：
- `consul connect proxy` **不需要预注册** — 直接启动即可，自动注册服务
- `-register` flag 让 proxy 自行注册 sidecar-proxy 到 Consul agent
- `network_mode: service:X` + 不同端口（内/外分离）**无端口冲突**

---

## 2. 证书与服务身份

### 2.1 信任根：Consul 内置 CA

不需要外部 CA / Vault。Consul Connect 启用后自动生成自签名根证书。docker-compose 场景下内置 CA 足够；生产环境将来可切换到 Vault PKI 或外部 CA（`consul connect ca set-config` 热切换），本期不涉及。

### 2.2 服务身份：SPIFFE ID

每个服务在 Consul 注册时自动获得 SPIFFE 身份：

```
spiffe://<cluster>/.consul/ns/default/dc/<datacenter>/svc/<service-name>
```

身份完全由服务名派生，无需应用代码感知——Envoy sidecar 在 mTLS 握手时自动出示和校验。

### 2.3 证书轮换：全自动

Envoy sidecar 通过 Consul agent 的 SDS（Secret Discovery Service）端点获取 leaf 证书。Consul 默认证书 TTL 72h，到期前自动轮换。应用进程全程不接触证书文件，零代码改动。

### 2.4 服务注册改造

现有 `ConsulRegistrar`（`libs/consul-registrar/src/lib.rs`）PUT `/v1/agent/service/register` 时只注册普通服务。扩展注册 payload，加 `connect` 字段让 Consul 自动创建 sidecar proxy 注册：

```rust
#[derive(Serialize)]
struct AgentServiceRegistration {
    id: String,
    name: String,
    address: String,
    port: u16,
    check: AgentServiceCheck,
    connect: ConnectConfig,    // 新增
}

#[derive(Serialize)]
struct ConnectConfig {
    sidecar_service: SidecarConfig,
}
```

Consul agent 注册服务时同时登记 `<service>-sidecar-proxy` proxy 实例，Envoy 启动时 `consul connect envoy -sidecar-for <service-id>` 绑定。

### 2.5 Sidecar 容器部署模式

docker-compose 里实现 sidecar 共享 localhost 的标准做法——sidecar 容器拥有网络栈（加入 `superpowers` network），app 容器通过 `network_mode: "service:<svc>-sidecar"` 共享它：

```yaml
iam-sidecar:          # envoy sidecar 容器，绑定 :8080（网络端口）
  image: consul:1.15
  command: consul connect envoy -sidecar-for iam-service ...
  networks:
    - superpowers

iam-service:          # app 容器，绑定 127.0.0.1:18080
  ...
  network_mode: "service:iam-sidecar"   # 共享 sidecar 网络栈
```

app 和 sidecar 共享 localhost：app 绑 `127.0.0.1:18080`，sidecar 绑 `:8080`，sidecar 转发到 `127.0.0.1:18080`。

---

## 3. 意图矩阵与授权策略（替代 service-token）

### 3.1 Consul Connect 意图（Intentions）

意图是 mesh 层的授权策略，声明"源服务 → 目标服务：allow / deny"，由 Envoy sidecar 在 mTLS 握手时强制执行，应用完全无感知。

### 3.2 意图矩阵

| 源服务 | 目标服务 | 动作 | 理由 |
|--------|----------|------|------|
| api-gateway | iam-service | allow | 路由转发 |
| api-gateway | token-service | allow | 路由转发 |
| api-gateway | notification-service | allow | 路由转发 |
| api-gateway | audit-service | allow | 路由转发 |
| api-gateway | config-service | allow | 路由转发 |
| api-gateway | file-service | allow | 路由转发 |
| api-gateway | prue_backend | allow | 路由转发 |
| token-service | iam-service | allow | `fetch_json` 取 claims |
| notification-service | iam-service | allow | `/iam/internal/users/by-{role,dept}` |
| **其余所有组合** | | **deny** | 默认拒绝 |

**默认拒绝姿态**：Consul Connect 默认策略设为 `deny`，只有上表显式 `allow` 的才放行。

### 3.3 service-token 一次性退役

项目无在线流量、无需向后兼容，直接一次性移除 `require_service_jwt`（`services/iam-service/src/middleware/service_jwt.rs`），无需并行观察期。

现有中间件做了两件事：

| 现有检查 | 用途 | 去向 |
|----------|------|------|
| `aud` 含 `"service"` | 服务身份认证 | **移除**——mesh 意图在传输层替代，mTLS 证书不可伪造 |
| `tid` 存在 | 多租户校验 | **迁移到 `TenantScope` extractor**（Phase 8.1 已建），走 `Principal.tenant_id` |

退役操作：
1. 删 `services/iam-service/src/middleware/service_jwt.rs` + 路由挂载
2. `/iam/internal/*` 改挂 `TenantScope` extractor 做 tenant 校验
3. 删 `NOTIF_SERVICE_TOKEN_INITIAL` 等 service-token 签发机制
4. 删 `internal_users_test.rs` 里 `mint_service_token` 相关测试 fixture

### 3.4 意图管理方式

用 Consul CLI 声明式管理，写入 `scripts/consul-intentions.sh` 脚本幂等执行：

```bash
consul intention create -allow api-gateway iam-service
consul intention create -allow token-service iam-service
# ...
consul intention create -deny '*' '*'
```

脚本纳入 `infrastructure/` 版本控制，`docker-compose up` 后执行一次。

---

## 4. 迁移与落地步骤（一次性切换）

项目无在线流量、无需向后兼容，采用一次性全量切换。

### 步骤 1：基础设施层 — Consul Connect 启用

- `docker-compose-infra.yml`：取消注释 `connect.hcl`，Consul 启用 Connect
- Consul 内置 CA 初始化（`consul connect ca init`）

### 步骤 2：库层 — `ConsulRegistrar` 扩展

- `libs/consul-registrar`：注册 payload 加 `connect.sidecar_service` 字段
- 单测覆盖新字段序列化

### 步骤 3：每个服务的 docker-compose + 应用绑定地址

- 每个服务加一个 `*-sidecar` 容器（`consul:1.15`，跑 `consul connect envoy -sidecar-for <svc>`）
- app 容器改 `network_mode: "service:<svc>-sidecar"`，应用绑定地址从 `0.0.0.0:8080` 改 `127.0.0.1:18080`
- 出站调用方（gateway `egress_map`、token/notification 的 `iam_base_url`）地址改本地 sidecar proxy 端口

### 步骤 4：意图矩阵应用

- `scripts/consul-intentions.sh` 幂等执行，写入第 3 节全量意图 + 默认 deny

### 步骤 5：service-token 一次性移除

- 删 `services/iam-service/src/middleware/service_jwt.rs` + 路由挂载
- `/iam/internal/*` 改挂 `TenantScope` extractor
- 删 `NOTIF_SERVICE_TOKEN_INITIAL` 等 service-token 签发机制
- 删 `internal_users_test.rs` 里 `mint_service_token` 相关测试 fixture

### 步骤 6：验证

- `docker-compose up` 全栈，sidecar 全部 healthy
- e2e 全量回归（`node scripts/run-e2e-local.js`）零失败
- 手工验证：非授权服务访问 iam-service → 被 mesh 拒绝

### 不在本期范围

- outbox-publisher / ntf-outbox-publisher 等 CLI binary——不接 HTTP，只连 RabbitMQ + MySQL，无需 mesh
- RabbitMQ / MySQL / Redis 的 mTLS——基础设施层，本期不动

---

## 5. 可观测性与测试验证

### 5.1 Mesh metrics 接入现有 Prometheus

现有 `infrastructure/config/prometheus/prometheus.yml` 已有 `consul_sd_configs`，通过 Consul 服务发现抓取所有注册服务的 `/metrics`。Consul Connect sidecar 注册为 `<svc>-sidecar-proxy` 后会被自动发现，无需额外配置 Prometheus job。

Envoy stats 通过 Consul agent 暴露在 `:8502/metrics`（已有 `8502` 端口映射），Consul 自动聚合所有 sidecar 的 mTLS / 意图指标。

### 5.2 关键指标（Grafana 面板）

| 指标 | 含义 | 告警阈值 |
|------|------|----------|
| `envoy.cluster.upstream_cx_connect_fail` | 上游连接失败（mTLS 握手失败） | >0 持续 1min |
| `consul.connect.intention_deny` | 意图拒绝计数 | >0 即告警 |
| `envoy.server.live` | sidecar 存活 | =0 即告警 |
| `envoy.process.virtual_memory` | sidecar 内存占用 | 观察基线，确认 <80MB |

### 5.3 测试验证

| 层级 | 范围 | 方式 |
|------|------|------|
| 单元测试 | `ConsulRegistrar` 的 `connect.sidecar_service` 字段序列化 | `libs/consul-registrar/tests/`，断言 JSON payload 含 `Connect` 字段 |
| 集成测试 | sidecar 启动 + mTLS 握手成功 | docker-compose 全栈起，`nc -z <svc>:8080` 验证 sidecar 端口可达 |
| e2e 回归 | 全量 API 流程零失败 | `node scripts/run-e2e-local.js`（现有 runner，覆盖 gateway→下游→事件总线全链路） |
| 安全验证 | 非授权服务被 mesh 拒绝 | `test-case/51-mtls-mesh.md`：起一个临时容器无 sidecar，直接连 iam-service:8080 → 连接被拒 |
| 意图验证 | 默认 deny 生效 | `test-case/51-mtls-mesh.md`：file-service 尝试调 iam-service → 意图拒绝 |

### 5.4 test-case 文档

新建 `test-case/51-mtls-mesh.md`，记录手工验收步骤（与现有 `41`~`65` 系列格式一致）。

---

## 风险与约束

| 项 | 说明 | 缓解 |
|---|---|---|---|
| Consul dev 模式无持久化 | 重启后 CA / 意图丢失 | docker-compose dev 环境可接受；生产需 Consul cluster（Phase 7） |
| `network_mode: "service:X"` 改变容器网络模型 | app 容器不再直接加入 `superpowers` network，端口映射行为变化 | sidecar 容器持有网络栈，端口映射移到 sidecar 容器 |
| ~~Envoy 镜像版本~~ | ~~Consul 1.15 需匹配的 Envoy 版本~~ | **已解决**：改用 consul connect proxy（内置），无需 envoy |
| 出站 sidecar 端口分配 | 每个上游服务需一个本地 proxy 端口 | 统一分配：iam=9081, token=9082, notification=9083, audit=9084, prue=9087 |
| e2e runner 可能需要适配 | sidecar 启动顺序、健康检查依赖 | docker-compose `depends_on` + sidecar 自动注册 |
| ConsulRegistrar fallback host | UDP probe 失败后 fallback 到 service name | 需 rebuild libs/consul-registrar 改为 127.0.0.1（已知问题 §6.3） |
| api-gateway 启动时序 | 需要 token-service JLWS 在启动时就绪 | 当前：api-gateway 重启一次后正常（token-service 已就绪）；长期：延迟 JLWS fetch 到 runtime |

---
## 6. 实施结果与遗留

### 6.1 验证结果（2026-07-04）

| 验收项 | 状态 | 说明 |
|--------|------|------|
| consul connect proxy 运行 | ✅ | 6 个服务各一个 proxy，全部 PID=1 运行 |
| 服务注册 | ✅ | 所有服务在 Consul Catalog 可见 |
| mTLS 生效 | ✅ | curl 直连 → "Empty reply from server"（需 TLS 握手） |
| 意图部署 | ✅ | consul-intentions.sh 执行成功，9 allow + 1 deny-all |
| e2e - 用户注册 | ✅ | `POST /iam/auth/register` → 201 Created |
| e2e - login | ⚠️ | api-gateway JWT 中间件阻塞 public 路由（预存 bug） |

### 6.2 各服务注册详情

| 服务 | 内端口 | 外端口 | upstream 出站 | 说明 |
|------|--------|--------|--------------|------|
| api-gateway | 18000 | 3000 | iam/token/notification/audit/prue/file/config | gateways + sidecar |
| iam-service | 18081 | 9081 | 无 | 纯入站 |
| token-service | 18082 | 9082 | iam-service 9081 | 调用 iam 校验用户 |
| notification-service | 18083 | 9083 | iam-service 9081 | 调用 iam internal API |
| audit-service | 18084 | 9084 | 无 | 纯入站（消费 RabbitMQ） |
| prue_backend | 18087 | 9087 | 无 | 纯入站 |
| config-service | 8081 | — | — | 无 mesh（直连 bridge） |
| file-service | 8082 | — | — | 无 mesh（直连 bridge） |

### 6.3 遗留问题

**1. ConsulRegistrar 健康检查 critical（需 rebuild）**

`libs/consul-registrar/src/lib.rs` 的 `discover_advertise_host()` 在 UDP probe 失败时 fallback 到 service name（如 `"iam-service"`），导致 Consul TCP 健康检查无法通过 Docker DNS 解析 hostname。

修复方向：将 fallback 改为 `"127.0.0.1"`（或 `"<svc>-sidecar"`）。
```rust
// ponytail: 改这里, 然后 rebuild 所有服务
fn discover_advertise_host(...) -> String {
    ...
    // 当前: fallback.to_string()  // e.g. "iam-service"
    // 改为: "127.0.0.1".to_string()  // network_mode: service:X 场景
}
```

**2. api-gateway JWT 中间件阻塞 public 路由**

`api-gateway/src/main.rs` 的 JWT middleware 对 `public=true` 的路由（如 `/v1/iam/auth/login`、`/v1/iam/auth/register`）返回 401。这导致无法通过 gateway 外部端口测试完整链路。通过 Docker DNS 直连内端口可绕开此问题，业务功能已验证。

---

## 关联入口

- 总路线图：`docs/superpowers/plans/2026-06-25-overall-roadmap.md` § Phase 5
- 现有 Consul 注册库：`libs/consul-registrar/src/lib.rs`
- 现有 service-token 中间件（待移除）：`services/iam-service/src/middleware/service_jwt.rs`
- 网关 egress 配置：`infrastructure/config/api-gateway/api-gateway.toml`
- Prometheus 配置：`infrastructure/config/prometheus/prometheus.yml`
- 多租户 extractor（tenant 校验迁移目标）：`libs/jwt-ctx`（`TenantScope`）、`libs/common-db`（`WithTenantScope`）
