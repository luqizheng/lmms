# Phase 5-A-2：放弃 Consul Connect，改用 gliderlabs/registrator 项目级注册

**日期**：2026-07-07 · 起草
**状态**：草案待评审
**关联**：
- 现行 mesh 实现 `docs/superpowers/specs/2026-07-03-phase5a-consul-connect-mtls-design.md`（本 spec 反对 / 替代其核心方案）
- 数据平面替代 `docs/superpowers/plans/2026-06-29-phase5a-mtls-sidecar.md`（consul-dataplane 迁移，也被本 spec 取消）
- `infrastructure/` 仓库当前 working tree：`token-service-sidecar` 与 5 个其他 sidecar 已经因为 `consul-dataplane` 镜像缺失 `consul` 二进制而重启循环，需在本 spec 落地时一并清理

---

## 0. 一句话

> 撤掉 Consul Connect（sidecar / intentions / connect proxy / consul-dataplane），用单一 `gliderlabs/registrator` 容器监听 `/var/run/docker.sock`，从容器 labels 自动注册服务到 Consul 的 catalog。**业务代码完全不动 Consul**，server 仍用 Consul 只用其 DNS / catalog / K/V / 健康检查。服务间走 plain HTTP（开发）+ `Authorization: Bearer <jwt>` 透传。

---

## 0.5. 决策路径回溯（brainstorming trace）

> 本节专门记录「为什么这样定」——把对话里的选项、否决、确认点都串起来，方便之后 review。下游实施时**无需重读对话**，照本节给出的理由即可。

### 0.5.1 时间线

| # | 触发 | 问题 / 选项 | 决定 | 理由 |
|---|---|---|---|---|
| 1 | 2026-07-06 token-service-sidecar 日志：`infrastructure-consul-1:8502: no such host` | `DP_CONSUL_ADDRESSES` 把端口塞进 DNS 名，go resolver 把整个 `host:port` 当 hostname 解析 | 把端口从 `DP_CONSUL_ADDRESSES` 抽到 `DP_CONSUL_GRPC_PORT` | consul-dataplane 文档明确 `-addresses` 只吃 DNS name，端口走独立 flag |
| 2 | 同上 | 改完 env 后，新错 "tls: first record does not look like a TLS handshake" | dev 模式加 `DP_TLS_DISABLED: "true"` | consul-dataplane 默认要 TLS，`agent -dev` 不出证书 |
| 3 | 同上 | 改完 TLS 后，新错 "node not found" | 手工 `PUT /v1/catalog/register` 注册了 `token-service-sidecar` 节点 | consul-dataplane 假设节点已存在（K8s/ECS 由平台注册），Compose 没有平台 |
| 4 | 同上 | 仍报错 "Service not found" | 手工再注册 `token-service-sidecar-proxy` connect-proxy service | dataplane 要节点 + 服务双 registration |
| 5 | 用户：「回滚吧」 | consul-dataplane 路径太麻烦，回滚到 `consul connect proxy` 同其他 5 个 sidecar | 执行回滚：token-sidecar 改回 `consul:1.15` + 激活 `consul connect proxy` command | 让 token-service 跟其他 sidecar 同款，立刻可用 |
| 6 | 用户报：「23:47:15 consul addresses not specified」 | 单独看是 token-sidecar 的问题 | 全仓 `docker ps` 验证：6 个 sidecar 全 `Restarting(1)` | `git diff HEAD` 显示**所有 6 个 sidecar 都被改 image → consul-dataplane 但 command 没改**，因为我自己的 token 回滚 edit 漏了 image 一行，其他 5 个是 working tree 里未提交改动 |
| 7 | 同上 | 怎么补 | 全部 image 改回 `consul:1.15` | 这是用户一次未提交实验的副作用，需要一次性收尾 |
| 8 | 用户：「regitrator 是否更简单」 | 注册器 vs sidecar proxy | 否 —— registrator 跟 Consul Connect 是两套模型，硬塞会造成 Connect-aware 客户端拿 mTLS proxy，其他客户端拿明文 | docmuntation 上 registrator 是个 plain-service 注册器，不懂 `Kind: connect-proxy` |
| 9 | 用户：「Consul 主动发起链接，consul-intentions.sh 还需要吗？」 | 三种架构：撤 mesh / 留 mesh + dataplane / mesh gateway | （讨论中） | —— |
| 10 | AI 自荐：推荐路径 A —— 撤 mesh | 给了三个理由：仓库名 + 服务规模 + 已有 JWT 路径 |  |  |
| 11 | 用户：「采用 A + 用 registrator sidecar」 | registrator 是 sidecar 还是项目级？ | 项目级 1 个容器 + 每服务 ~7 行 labels，业务零改 | registrator 原本就是 host-wide 监听 docker.sock 的设计，per-service sidecar 反而麻烦 |
| 12 | 用户：「出 plan」 | 走 writing-plans 写实施计划 | （用户改主意：先写 spec 含决策过程） | spec 是 contract，plan 是 execution，分清 |
| 13 | 用户：「根据上面讨论先写spec，把决策过程写入，方便之后我回顾」（本次） | spec 补全决策追溯 | 在本 spec 加 §0.5 / 扩 §12 | 用户明示 spec 是长期 review 入口，把判断锚点固化 |

### 0.5.2 三次「回滚/换轨」的根因 & 教训

| 关键回滚 | 表层原因 | 深层原因 | 教训 |
|---|---|---|---|
| consul-dataplane → connect proxy（步骤 5） | 节点 / 服务要预注册，路径繁 | consul-dataplane 设计假设 K8s/ECS 控制面帮忙注册 | Compose 生态用 `consul connect proxy -register` 是 1 token 的事，dataplane 是为跨 orchestration 设计的 |
| consul-dataplane image 行漏修（步骤 6） | 我回滚 token sidecar 的 edit 漏了 image 一行 | 老 session 的 partial edit 没做一致性校验 | 复杂多 service 改动前做 **`docker compose config` 渲染 diff vs 期望** 比肉眼读快 |
| 5 个 sidecar 也是 consul-dataplane image 但 command 没变（步骤 6） | 用户未提交的实验改动 | 多人多分支改同一个 compose，没有 PR / pre-commit 检查 | PR / CI 拦截 compose config 漂移（spec 之外的下一步） |

### 0.5.3 三个否决选项的论据存档

**B · 保留 Connect（consul-dataplane 迁完）**：
- ❌ consul-dataplane 镜像要 Node + connect-proxy Service 预注册（步骤 3、4 实证）
- ❌ dev 模式还要 `DP_TLS_DISABLED` 或自签 CA
- ❌ 6×64MB sidecar 内存债 vs 仓库名 `low-memory-micro-services`
- ❌ 服务身份（SPIFFE）当前没有任何**外部消费者**来验证（mesh 的核心收益点）
- ❌ 同样的 mesh 在这次 session 已经出现 2 次 sidecar 循环（dataplane、connect proxy 命令 vs 镜像）

**C · 保留 mesh，替换 Consul 为 Nacos / Polaris**：
- ❌ 项目阶段（AGENTS.md「无生产数据、无向后兼容」）—— 主要风险在副作用
- ❌ 当前的 `libs/consul-registrar` 6 改动 + `consul` 5 个 CRD 都得重来
- ❌ 收益主要是「换个东西」，mesh 本身的成本（sidecar / 调试 / mTLS 排障）仍然在

**A · 撤 mesh**：见 §0.5.4 的采纳理由。

### 0.5.4 路径 A 采纳的理由（按权重降序）

1. **仓库立意**：`low-memory-micro-services` — 6×64M sidecar 与此对立。registrator 一个 ~10M 容器顶替所有 sidecar。
2. **已具备替代品**：
   - `libs/jwt-ctx::Principal` 已经解码 Bearer token
   - `digua_common_event::MySqlOutboxRepo` 已经把跨服务协同下沉到 RabbitMQ events
   - `permissions.yaml` 已经把应用层 RBAC 集中化
3. **正交化范围**：mesh 撤掉只动 7 个 service main + 1 lib + 1 connect.hcl + 1 sidecar 文件 + 1 intentions 脚本；不在业务语境内做语义改动。
4. **服务规模**：7 个 service，mesh 投资回报率在小拓扑不显著。
5. **已经验证过的 frictions**：本 session 已经踩过 dataplane 预注册、dev 模式 TLS、镜像不一致导致的 6 sidecar 重启循环 —— 这些都是「保留 mesh」路线的实际摩擦样本。
6. **AGENTS.md 允许**：开发阶段、无生产数据、无需向后兼容 —— 一次性 switch 不需渐进式 rollout。

### 0.5.5 自检 checklist（spec review 时的对照）

- [x] 决策点 1（路径 ABC）有答案 + 理由
- [x] 每个被否决选项有反对论据
- [x] 「不做什么」显式列出（§2.4 + §0.5.3）
- [x] 改动影响面按文件列（§5.2）
- [x] 每个改动能定位到一个 git commit
- [x] 验证步骤可执行（§8）
- [x] 回滚路径存在（§9）
- [x] 风险与缓解成对（§10）

### 0.5.6 仍未解决的开放问题（spec 签后下一轮谈）

| 问题 | 暂定答案 | 何时拍板 |
|---|---|---|
| registrator 镜像 tag 固化到 sha256 | 用 `:master` 跑通后固化 | 实施 step 1 验证完 |
| rabbitmq events 化清单里 `notification → iam.users lookup` 是否要事件化 | 否，本 spec 范围内只做 Authorization 透传 | 后续 spec `phase5b-event-sync` |
| 是否还要 dashboard 用 Consul metrics | 当前没有 panel 接入，下一 phase 同步 | 实施后看 dashboard plan |
| `docker-example` 是否一定同步 | 是，与实际仓库同步是新人 copy 起点 | 实施 step 7 |

---

## 1. 背景与现状

### 1.1 今日痛点（已发生）

2026-07-06~07 排除 token-service-sidecar DNS 报错的过程顺带暴露：

- 6 个 sidecar 的 `image:` 在 working tree 里被改成 `hashicorp/consul-dataplane:2.0.1`，但 `command:` 仍然调用 `consul connect proxy`，镜像里没有 `consul` 二进制，所以全部 sidecar 进入重启循环（`consul addresses not specified`）
- consul-dataplane 镜像需要预先注册 Consul Node + `Kind: connect-proxy` Service 才能 bootstrap（dev 模式 manual 流程已经在实验中被验证），摩擦大
- 业务代码经过 `libs/consul-registrar` 直接 PUT `/v1/agent/service/register`，且 6 个 service 都启用了 `ConnectConfig { sidecar_service: SidecarConfig {...} }`

### 1.2 mesh 价值不高

- **服务规模**：7 个 Rust service，无跨团队、无跨语言、无多 DC
- **服务身份**：mesh 提供的 SPIFFE identity 没有任何外部消费者（SaaS、partner）来验证
- **服务间 authn/digua_authz**：i）网关入口已签 `Authorization: Bearer <jwt>`，`libs/jwt-ctx::Principal` 解码信任网关验签；ii）`/iam/internal/*` 端点已经有 `require_service_token`（`aud: "service"`）应用层身份认证
- **仓库命名**：`low-memory-micro-services` —— mesh sidecar 6×64MB ≈400MB 与仓库立意冲突
- **观测**：当前的 `connect.connect_*` Prometheus 指标没有采集面板接（log search 中无）

### 1.3 决策（2026-07-07）

| 选项 | 状态 |
|---|---|
| A. **撤 Connect + 用 registrator** | **采纳** |
| B. 保留 Connect（consul-dataplane 修好 + node pre-register 自动化） | 拒绝 — 见 §1.2 |
| C. 保留 mesh 但换 Consul 替换（Nacos / Polaris） | 拒绝 — 项目阶段付不起切换税 |

---

## 2. 目标架构

### 2.1 拓扑

```
docker compose -p infrastructure up -d
   ↓
[superpowers 网络]
├── consul (existing, dev mode -dev)
│   ├── catalog：服务注册表（registrator 写入）
│   ├── DNS：<svc>.service.consul 解析
│   ├── K/V：当前没用，未来 config-service 配合
│   └── health：HTTP/TCP check (registrator 创建)
├── mysql / redis / rabbitmq
├── registrator ← 本 spec 新增
│   ├── watch docker.sock
│   ├── 读 labels → PUT /v1/agent/service/register
│   └── 容器 stop → 自动 deregister
├── iam-service      (port 8081, labels)
├── token-service    (port 8082, labels)
├── notification-service (port 9083 → 8083, 见 §7)
├── audit-service    (port 8084, labels)
├── config-service   (port 8081, 见 §7 端口冲突 → 8085)
├── file-service     (port 8086, 见 §7)
├── prue_backend     (port 8087, labels)
└── api-gateway      (port 3000, labels)
```

### 2.2 网络端口最终态（从 mesh 内外端口分离回到单层端口）

| Service | 旧（mesh） | 新（本 spec） |
|---|---|---|
| api-gateway | 3000 / 18000 | 3000 |
| iam-service | 9081 / 18081 | 8081 |
| token-service | 9082 / 18082 | 8082 |
| notification-service | 9083 / 18083 | 8083 |
| audit-service | 9084 / 18084 | 8084 |
| config-service | 8081（直） | 8085（避让 iam） |
| file-service | 8082（直） | 8086（避让 token） |
| prue_backend | 9087 / 18087 | 8087 |

> ⚠️ 8081 是 gateway 转 iam 用的端口，config-service 当前占 8081 host port，本 spec 把 config-service 改到 8085。如不挪，host 端口冲突；容器内端口仍可继续 8081，但**外部访问靠 gateway 转发**，不影响业务。详见 §7。

### 2.3 服务间调用拓扑

```
              Authorization: Bearer <jwt>
            ┌─────────────────────────────┐
            ▼                             │
api-gateway ──▶ iam-service               │
            ──▶ token-service             │
            ──▶ notification-service ──▶ iam-service
            ──▶ audit-service             │
            ──▶ config-service            │
            ──▶ file-service              │
            ──▶ prue_backend              │
                                          │
token-service ─────────────────────▶ iam-service  (内部 fetch_claims)
notification-service ─────────────▶ iam-service  (内部 users lookup)
```

所有内部调用：
- HTTP 明文（开发） / 内部由 nginx 或后续 LB 终结 TLS（不在本 spec 范围）
- 携带 `Authorization: Bearer <jwt>`（网关给当前用户签的 jwt）
- iam 端对 `aud` 不强制 user，仅校验 `exp`（同一把密钥），所以内部调用等同于「用户身份在代理」

### 2.4 不再需要的东西

| 组件 | 删除路径 |
|---|---|
| 6 个 sidecar 服务 | `docker-compose-services.yml` 删 `*-sidecar` services |
| `api-gateway-sidecar` 同上 | 同上 |
| `consul-init` 服务 | `docker-compose-infra.yml` 删 |
| `connect { enabled = true }` 块 | `infrastructure/config/consul/connect.hcl` 删 |
| `scripts/consul-intentions.sh` | 删 |
| libs/consul-registrar 中 `ConnectConfig / SidecarConfig` | 见 §6 |
| service 端口 180xx 内外分离 | 业务 `SERVER_PORT` 全回 808x |

---

## 3. 注册模型 — registrator 而不是 consul connect proxy

### 3.1 registrator 服务定义（最小）

新增到 `infrastructure/docker-compose-infra.yml`：

```yaml
  registrator:
    image: gliderlabs/registrator:master   # 后续可固化到 sha256 digest
    depends_on:
      consul:
        condition: service_healthy
    volumes:
      - /var/run/docker.sock:/tmp/docker.sock
    environment:
      DOCKER_HOST: unix:///tmp/docker.sock
      REGISTRATOR_KEEP_RAW: "true"   # 见 §3.4
    command: ["consul://consul:8500"]
    restart: unless-stopped
    networks:
      - superpowers
```

### 3.2 标准 labels anchor（每个 service 复用）

```yaml
x-service-registrator-labels: &svc-reg-labels
  - "SERVICE_NAME=${SERVICE_NAME}"
  - "SERVICE_ID=${SERVICE_ID}"
  - "SERVICE_PORT=${SERVICE_PORT}"
  - "SERVICE_TAGS=backend,${SERVICE_NAME}"
  - "SERVICE_${SERVICE_PORT}_CHECK_HTTP=${SERVICE_HEALTH_PATH}"
  - "SERVICE_${SERVICE_PORT}_CHECK_INTERVAL=15s"
  - "SERVICE_${SERVICE_PORT}_CHECK_TIMEOUT=3s"
```

`.env` 里加：
```
SERVICE_NAME=
SERVICE_ID=
SERVICE_PORT=
SERVICE_HEALTH_PATH=
```

每个业务 service 在 `docker-compose-services.yml` 内覆盖这些 env，并在 `labels:` 引用 anchor。

### 3.3 健康检查策略

| Service | 路径 | 间隔 | 备注 |
|---|---|---|---|
| iam-service | `/health` | 15s | 现成，spec 上没改 |
| token-service | `/health` | 15s | |
| notification-service | `/health` | 30s | 慢冷启动 |
| audit-service | `/health` | 30s | |
| config-service | `/health` | 15s | |
| file-service | `/health` | 30s | |
| prue_backend | `/health` | 15s | |

### 3.4 自动注销 / 重对账

- registrator 默认 `-resync=0` 不轮询，本 spec 设 `-resync=30s` 每 30s 重新对一次容器 ↔ Consul catalog，应对 `docker-compose restart` / `docker kill` 后的瞬态不一致
- 容器 stop / kill：registrator 通过 Docker events 调 `/v1/agent/service/deregister`
- 容器 restart 同名复用 Service ID → 注册幂等

### 3.5 没用 registrator 的字段

- ❌ `ConnectConfig / SidecarConfig`：mesh 概念，注册时不输出
- ❌ `SERVICE_<PORT>_CHECK_TCP`：项目都已有 HTTP `/health`，不需要再起 TCP check
- ❌ `-tags` 全局 tag 模板：本项目每个 service 单独打 tag，不需要模板

---

## 4. 服务间调用 —— 直连 + Authorization 透传

### 4.1 `Authorization` 透传规则

- 网关已经给用户签的 JWT（含 `sub / roles / expires_at` 等）由 `libs/jwt-ctx::Principal` 解码
- 内部调用无需新签 service token。复制用户的 `Authorization: Bearer <user-jwt>` 给内部调用即可
- iam-service 当前已有 service token 校验（`/iam/internal/*`），收紧到：
  - 网关入口：`Audience::User` 即可
  - 内部调用（`/iam/internal/*`）：`Audience::Service` 或 `Audience::Any`（user JWT 也认）

### 4.2 改动表（业务代码最小集）

| 调用点 | 当前 | 目标 |
|---|---|---|
| `api-gateway` upstream URL | `iam-service:9081` (proxy) | `iam-service:8081` |
| `token-service` 调 iam | sidecar proxy | `iam-service:8081/iam/internal/users/:id/claims`，带 `Authorization: Bearer <user-jwt>` |
| `notification-service` 调 iam | sidecar proxy | `iam-service:8081/iam/internal/users/by-{role,dept}`，带 `Authorization` |
| `audit-service` | rabbitmq events only，零直连 | 不动 |
| `services/iam-service` `Audience` enum | 仅 `User / Service` | 加 `Any` 或开放 `User` 对 `/iam/internal/*` |

### 4.3 不再使用的旧结构

- `libs/consul-registrar` 中 `ConnectConfig / SidecarConfig` 字段、`with_sidecar_config` 方法、`tests/register_payload_test.rs` 中 connect 相关用例 —— 全部删除
- `api-gateway.toml` 中 `egress_map`（mesh 下游解析）—— 改回 simple upstream（按 host:port）

---

## 5. 改动清单

### 5.1 文件删除 / 不再使用

| 文件 | 处理 |
|---|---|
| `scripts/consul-intentions.sh` | 删 |
| `docs/superpowers/plans/2026-06-29-phase5a-mtls-sidecar.md` | 标 archived / superseded |
| `docs/superpowers/specs/2026-07-03-phase5a-consul-connect-mtls-design.md` | 在 §0 加 superseded 标记，路径不删（保留作历史） |
| `docker-example/` 中所有 sidecar 样例（如果存在） | 跟实际仓库对齐，等价展开 |

### 5.2 文件修改

| 文件 | 内容 |
|---|---|
| `infrastructure/docker-compose-infra.yml` | 加 `registrator` 服务；删 `consul-init` |
| `infrastructure/docker-compose-services.yml` | 删全部 7 个 `*-sidecar` 服务；每个 business service 加 `labels:` 引用 anchor；删 `network_mode: "service:X-sidecar"` |
| `infrastructure/config/consul/connect.hcl` | 删 `connect { enabled = true }` 块；保留 `datacenter / data_dir / ports.grpc` |
| `infrastructure/.env` | 加 `SERVICE_*` 占位（见 §3.2） |
| `services/<each>/src/main.rs` | 删 `ConsulRegistrar::new().with_sidecar_config(...).register_service(...)` 调用块 |
| `services/<each>/src/config.rs` | `SERVER_PORT` / `SERVER_ADDR` 改回 808x（去掉 18xxx） |
| `services/token-service/src/issuer.rs` + `tests/issuer_test.rs` | 内部 fetch_claims 改直连 + Authorization 透传 |
| `services/notification-service/src/main.rs` | 同上 |
| `services/iam-service/src/main.rs` | `Audience::Service` 校验逻辑保留，或扩展为 `Audience::Any` |
| `services/api-gateway/src/main.rs` + `config.rs` + `router.rs` | `egress_map` 改回普通 upstream；删 `upstream_check_authorized` 类的 mesh 适配 |
| `services/api-gateway/config/api-gateway.toml` | 上游 URL 回到真服务端口 |
| `libs/consul-registrar/src/lib.rs` | 删 `ConnectConfig / SidecarConfig / with_sidecar_config` 字段；保留 K/V 健康报告（如还在用） |
| `libs/consul-registrar/tests/register_payload_test.rs` | 删 connect 测试用例 |
| `docs/superpowers/plans/TODO.md` / `docs-site/ai-context.md` | 更新引用 |

### 5.3 文件新建

| 文件 | 内容 |
|---|---|
| 无 | registrator 通过 labels 驱动，不需要新配置 schema |

---

## 6. libs/consul-registrar 收尾

### 6.1 现行职责

```rust
pub struct ConsulRegistrar {
    service_id: String,
    service_name: String,
    address: String,
    port: u16,
    upstreams: Vec<UpstreamSpec>,
    sidecar_config: serde_json::Map<String, serde_json::Value>,
    // ...
}

impl ConsulRegistrar {
    pub fn new(...) -> Self { ... }
    pub fn with_upstream(...) -> Self { ... }
    pub fn with_sidecar_config(...) -> Self { ... }   // ← 删
    pub async fn register_service(&self) -> Result<(), Error> {
        // PUT /v1/agent/service/register with connect: { sidecar_service: {...} }
        // ← 改为：什么都不做（业务代码移除调用），或保留 register 但不带 connect
    }
}
```

### 6.2 处理方案

方案 **A**：彻底删除 `libs/consul-registrar` crate + 7 处 `Cargo.toml` workspace 引用。**业务代码删干净**调用方。

方案 **B**：保留 crate 但只做 `ConsulKv` / metrics，保持向前兼容（即 aggressive A 是 A，conservative B）。本 spec 推荐 **A** —— AGENTS.md「开发阶段、无生产数据、无需向后兼容」。

---

## 7. 端口决策细化

### 7.1 当前 host-published 端口布局（来自 compose）

| Service | ports section |
|---|---|
| `config-service` | `8081:8081` |
| `file-service` | `8082:8082` |
| `api-gateway` | `3000:3000` |
| `iam-service`（mesh） | `9081:9081` |
| `token-service`（mesh） | `9082:9082` |
| `notification-service`（mesh） | `9083:9083` |
| `audit-service`（mesh） | `9084:9084` |
| `prue_backend`（mesh） | `9087:9087` |
| `frontend` | `8080:80` |

### 7.2 mesh 前后对比

- mesh 内端口（18xxx）取消 → 业务回到 808x 直接绑
- 网关转发的 upstream 也跟着改回 `iam-service:8081` 等

### 7.3 host-published 端口冲突

- `config-service: 8081:8081` → 改成 `8081`（容器内），host 端改 `8085:8081`（消歧）
- `file-service: 8082:8082` → 同上 `8086:8082`
- 其他不变

注意：业务逻辑只用容器内端口（`iam-service:8081`），8085 / 8086 只为 host 端开发联调，不影响 gateway 转发。

### 7.4 实际改动（commit 一并修改）

```yaml
config-service:
  ports: ["8085:8081"]   # host 8085 → container 8081
file-service:
  ports: ["8086:8082"]
```

---

## 8. 验证步骤

### 8.1 单元 / 集成不变

- `pnpm run check:env` 不应失败
- 各 service 的 `cargo test --lib` 不变（删 mesh 后部分 mock 函数不再需要保留）

### 8.2 启动顺序冒烟

1. `docker compose -f infrastructure/docker-compose.yml up -d`
2. 等待 `consul` healthy
3. 等待 `registrator` 起来
4. 等业务 service 启动 → `consul catalog services` 应出现 8 个 `*-service` / `api-gateway` / `prue_backend` / `frontend`
5. `consul catalog nodes -service=iam-service` 应能解析到 iam 容器 IP

### 8.3 功能烟测

| 测项 | 期望 |
|---|---|
| 前端登录 | 网关→token→iam 全链路返回 200，JWT 含 user claims |
| token service 调 iam (`fetch_claims`) | 内部 Authorization 头一并发过去，iam 校验通过 |
| notification 调 iam (`by-role`) | 同上 |
| service killed | registrator 30s 内自动 deregister，consul catalog 删除对应 service |
| service 重新拉起 | 自动 re-register，idempotent |

### 8.4 内存收益

| Component | memory |
|---|---|
| 6× consul connect proxy @ ~10MB | -60MB（每个 10MB-12MB 估算；之前盘点是 64MB Envoy，按 §1.3 的 1.6 落入项目改回了 10MB 内置 TCP proxy） |
| registrator | +10MB |
| **净值** | **-50MB ~ -380MB**（按之前 vs Envoy 估算差异） |

---

## 9. 回滚路径

mesh 删干净后回滚困难（路径以 commit / spec 落地为锚）。应急路径：

1. **可快速回滚（commit 前）**：`git revert <commit>`
2. **业务已经在用 plain HTTP 时**：连通性靠 superpowers 网络 DNS，不需要回滚到 mesh 才能跑
3. **部分组件回滚（比如某 service 内部调用拿不到 Authorization）**：临时在中间放 nginx `proxy_set_header Authorization` 透传

---

## 10. 风险

| 风险 | 缓解 |
|---|---|
| registrator master tag 不稳定 | 用 `image: gliderlabs/registrator@sha256:...` 固定 digest；初版用 master 跑通再固化 |
| `/var/run/docker.sock` 暴露 | 仅 dev / 内网 CI；prod 改 rootless 或 `tecnativa/docker-socket-proxy` |
| 所有内部调用必须带 Authorization header 才能调 `/iam/internal/*` | 当前 service token 校验逻辑保留，开发期 ui 用户 JWT 直接复用即可 |
| 端口 8081 / 8082 host 冲突 | config-service / file-service host port 改为 8085 / 8086，对网关无影响 |
| 7 个 service 现有 `ConsulRegistrar` 调用，需要 7 处 main.rs 改动 | 集中在 §5.2 表里逐个改，单次 commit |
| `frontend → api-gateway` 的 8080 没变；其他 nginx / 反代配置不动 | 单测 / 烟测覆盖 |

---

## 11. 关联文档 / 后续 TODO

- 后续 PR：把 `docker-example` 同款拓扑同步更新（避免 copy-paste 误导新人）
- 后续 spec：`docs/superpowers/specs/2026-XX-XX-internal-jwt-passthrough-design.md`（Authorization 透传细节，单独 spec）
- 后续清理：`phase5b-tls-termination.md` —— 出 mesh 后 dev 仍可 HTTP；生产路径需补 alb / nginx 终结 TLS，下一个 phase 处理

---

## 12. 决策日志（按时间，未来新增接下面）

> 每条形如：`YYYY-MM-DD HH:MM+TZ — 摘要 — 影响节`。回溯大纲见 §0.5。

### 已发生

- 2026-07-06 23:12+0800 — `DP_CONSUL_ADDRESSES` 写错（带端口） → DNS 解析失败。修法见 §0.5 时间线 #1。
- 2026-07-06 23:30+0800 — consul-dataplane 与 dev 模式 Consul 的 TLS 不匹配 → 加 `DP_TLS_DISABLED`。见 §0.5 #2。
- 2026-07-06 23:47+0800 — 暴露：**所有 6 个 sidecar** 在 working tree 里有 image / command 不一致 defect，全部进 restart loop。批量 image 改回 `consul:1.15`。见 §0.5 #5-7。
- 2026-07-07 01:00+0800 — Agent 自荐：路径 A（撤 mesh + registrator）。理由存档 §0.5.4。
- 2026-07-07 02:00+0800 — 用户确认采用路径 A。
- 2026-07-07 04:00+0800 — spec 起草，`docs/superpowers/specs/2026-07-07-deprecate-consul-connect-use-registrator-design.md` v1。
- 2026-07-07 11:00+0800 — spec v2，扩 §0.5 决策路径回溯 + §12 决策日志填实（用户 review）。

### 待发生

- 2026-07-07 11:30+0800 — 用户 review spec，sign-off 后进入 writing-plans 实施 plan起草。
- 实施期 — 每完成一个 step 追加 entry，格式：`- HH:MM — <commit subject> — <影响 §X.Y>`。

### 长期 ADR 锚点

后续重大决策（>= 跨服务 / >= 替换基础设施组件 / >= 数据库结构）按相同格式追加在本节，§0.5 的对应行做 ←→ 互引，让 ADR 长期可追溯。

## 13. spec 自身的内部审稿记录

> 「spec self-review」框架要求在提交前自查：占位符 / 内部一致性 / 范围 / 歧义。下面是起草 / review 时的发现项。

| 检查 | 结论 |
|---|---|
| 占位符（TBD / TODO / 待定） | §0.5.6 列出 4 个「暂定答案 + 何时拍板」，不是占位符而是显式的开放问题（spec 不假装它们已经解决） |
| §7 端口表 vs §2.2 端口表 | 一致（§2.2 给目标态，§7 解释 host-published 冲突和迁移做法） |
| §5.2 文件改动清单 vs §4.2 改动表 | 一致（§4.2 抽象「调用点行为变更」，§5.2 把它们落到具体文件） |
| §6 libs/consul-registrar 删彻底 vs §4.2 服务级改动 | §6 说「推 A（彻底删除）」，§4.2 / §5.2 表格按 A 写；如果 reviewer 选 B（保守），要再扫一遍 |
| §3.2 SERVICE_PORT 引用占位变量 vs §7 实际端口 | §3.2 用了 `${SERVICE_*}` 占位符，§7 在每个 service 的 compose entry 用 literals 覆盖；不矛盾 |
| 范围 | 单 spec 覆盖单一架构转换（撤 mesh），无「跨子系统」问题；见头脑风暴 step 3「scope check」 |
| 歧义 | §4.1 说「复制用户的 Authorization 给内部调用」，未说明若上游是异步任务（无 user 上下文）怎么签 token；落到 §4.2 已限定为 2 个直接调用点（token→iam、notification→iam），都是同步且 user 上下文明确。内部 cron / outbox publisher 不在本 spec 范围（它们只连 mysql/rabbitmq） |

### 已知 reviewer 可能问的三个问题（先列了方便你批）

1. **「业务代码删 ConsulRegistrar 后，谁负责健康检查？」** → registrator 自动从 `SERVICE_<PORT>_CHECK_HTTP` 创建 Consul HTTP check（§3.3）。见 §3.4 重对账机制。
2. **「Config-service / file-service 迁到 8085 / 8086 后，host 端调试地址变了？」** → 开发联调 URL 改 `http://localhost:8085`，gateway 转发路径不变。详见 §7。
3. **「本地 DNS 还能解析 `iam-service` 吗？」** → 仍然通过 docker DNS（`superpowers` 网络 alias），registrator 用容器 IP 注册 Service.Address；服务互调仍然走 `iam-service:8081`。详见 §2.1 + §3.5。
