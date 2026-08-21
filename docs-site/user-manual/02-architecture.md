# 02. 架构总览

> 回答三个问题：有哪些服务、怎么分层、一次请求怎么走。
> 上次更新：2026-08-21（删除过时的 §2.6 Consul Connect 整段；注册 / 服务发现请改读 [`docs-site/guides/service-registration.md`](../guides/service-registration.md)）

---

## 2.1 服务地图（Phase 5-A 之后）

```
        browser / SDK
              │  :8080 (frontend dev)
              ▼
       ┌──────────────┐
       │   frontend   │  :80 in container
       └──────┬───────┘
              │  HTTP（明文；frontend 不在 mesh 里）
              ▼
       ┌────────────────────────────────────┐
       │ api-gateway 容器 (network_mode:    │
       │   service:api-gateway-sidecar)     │
       │                                    │
       │  api-gateway-sidecar ←──── ingress ──── :3000 (host)
       │   consul connect proxy             │
       │   -listen :3000  (allow_untlsed)   │
       │   -upstream iam/token/notification/│
       │            audit/prue/file/config │
       │            ──── egress ───►        │
       │                                    │
       │  api-gateway 主进程 :18000         │
       │   （127.0.0.1，共享 sidecar 网络栈）│
       └──────┬─────────────────────────────┘
              │  via api-gateway-sidecar upstream
              ▼
   ┌──────────────────────┐    ┌──────────────────────────┐
   │ token-service        │    │ iam-service              │
   │ sidecar :9082        │    │ sidecar :9081            │
   │  ↕ :18082 (主进程)   │    │  ↕ :18081 (主进程)       │
   └──────────────────────┘    └──────────────────────────┘
   ┌──────────────────────┐    ┌──────────────────────────┐
   │ notification-service │    │ audit-service            │
   │ sidecar :9083        │    │ sidecar :9084            │
   │  ↕ :18083 (主进程)   │    │  ↕ :18084 (主进程)       │
   └──────────────────────┘    └──────────────────────────┘
   ┌──────────────────────┐    ┌──────────────────────────┐
   │ prue_backend         │    │ file-service  :8082      │  ← 无 sidecar，明文
   │ sidecar :9087        │    │ config-service  :8081    │  ← 无 sidecar，明文
   │  ↕ :18087 (主进程)   │    └──────────────────────────┘
   └──────────────────────┘

   ┌────────────────────────────────────────┐
   │ 后台 worker（无 sidecar，无 HTTP 入口） │
   │   iam-outbox-publisher                 │  ← MySQL outbox → RabbitMQ
   │   notification-outbox-publisher        │  ← MySQL outbox → RabbitMQ
   └────────────────────────────────────────┘
```

**关键点**（以 `infrastructure/docker-compose-services.yml` 为准）：

- 6 个服务走 Consul Connect mesh：api-gateway、iam、token、notification、audit、prue_backend
- 2 个服务**不**走 mesh（明文 HTTP）：file-service、config-service
- 2 个 outbox publisher worker：iam、notification（无 mesh、无 HTTP）
- 每个 mesh 服务有**两个端口**：
  - **内端口** `180xx`：`127.0.0.1:180xx`，服务进程监听，**只在本 network namespace 内可达**
  - **外端口** `908x`（gateway 是 `3000`）：sidecar 的 `consul connect proxy` 监听，docker 映射到 host
- 主服务与 sidecar 共享网络栈（`network_mode: "service:<svc>-sidecar"`）
- api-gateway sidecar 显式 `allow_untlsed_inbound=true`（让浏览器 / e2e 不被 mTLS 握手拦截；其他 sidecar 行为参考 [Phase 5-A spec §1.6](../superpowers/specs/2026-07-03-phase5a-consul-connect-mtls-design.md)，该 spec 已被 Phase 5-A-2 撤销）

---

## 2.2 分层职责

| 层 | 职责 | 禁止 |
|----|------|------|
| **api-gateway + sidecar** | JWT 验签、Redis 黑名单、限流、注入 trace context、**mesh mTLS 出站代理** | 业务逻辑 |
| **token-service + sidecar** | 签发 / 刷新 / 吊销 JWT；通过 sidecar 调用 iam | 用户 CRUD |
| **iam-service + sidecar** | 身份 + RBAC + 部门树（纯入站 mesh） | 直接发 token |
| **notification-service + sidecar** | 公告 + WebSocket 推送；通过 sidecar 调用 iam | 鉴权 |
| | （Phase 5B-fu3：role/dept 接收人解析改本地 `ntf_user_projection` 投影 + RabbitMQ 事件，零同步 HTTP） | |
| **audit-service + sidecar** | 审计事件写入 + RabbitMQ 消费（纯入站 mesh） | 调业务服务 |
| **prue_backend + sidecar** | 菜单配置（纯入站 mesh） | 任何业务调用 |
| **file-service**（明文） | 文件存储 / 下载 | — |
| **config-service**（明文） | 通用配置中心（明文，内网可信） | — |
| **outbox publisher**（后台） | 轮询 MySQL outbox → 发到 RabbitMQ（事务性发件箱） | 对外暴露 |

**原则**：业务服务之间**不直接 HTTP 调用**，全部走 api-gateway 转发（`Authorization` 头透传）；service-to-service 走 Consul Connect mesh（mTLS）。

---

## 2.3 典型请求链路（含 sidecar 跳）

```
1. 浏览器 POST /v1/auth/login 带凭据
2. Nginx (:80) 代理到 api-gateway (:3000)
3. api-gateway-sidecar 容器（外端口 3000）：
   a. consul connect proxy 接收请求
   b. 转发明文到 127.0.0.1:18000（api-gateway 进程；共享 network namespace）
4. api-gateway 进程：
   a. 链路中间件：建根 span、抽取 W3C traceparent
   b. 鉴权中间件：校验 JWT（如需；通过 jwks_url 拉 JWKS）
   c. 限流中间件：登录接口 5 rps
   d. 代理 handler：查 egress_map → iam-service 127.0.0.1:9081
5. 出站经过 api-gateway-sidecar：
   a. consul connect proxy 通过 -upstream iam-service:9081 出站
   b. Consul Intentions 检查 api-gateway → iam-service 是否 allow
   c. 转发到 iam-service-sidecar :9081
6. iam-service-sidecar 容器（外端口 9081）：
   a. consul connect proxy 接收请求
   b. 转发明文到 127.0.0.1:18081（iam-service 进程；共享 network namespace）
7. iam-service 进程：
   a. 链路中间件：通过 traceparent 接 span
   b. 校验凭据，颁发 token
   c. 返回 access_token + refresh_token
8. 响应回流（5 次反向）；响应头带 traceparent，便于浏览器关联
9. span 上报 OTel collector（若设置了 OTEL_EXPORTER_OTLP_ENDPOINT）
10. Jaeger UI 展示完整链路：gateway-sidecar → gateway → iam-sidecar → iam
```

> 一笔请求至少穿过 **2 对 sidecar 跳**（gateway 进出 + iam 进出）。mesh 层是否启用 mTLS 终止由 Consul 1.15 `consul connect proxy` 默认行为决定，本仓库 api-gateway sidecar 显式 `allow_untlsed_inbound=true`（让浏览器直连 :3000 不被 mTLS 握手拦截），其他 sidecar 行为详见 [Phase 5-A spec §1.6](../superpowers/specs/2026-07-03-phase5a-consul-connect-mtls-design.md)。

---

## 2.4 8 项服务标准（ADR-002）

每个 Rust 服务必须具备：

- [ ] Dockerfile
- [ ] `AGENTS.md`（本服务约定）
- [ ] `README.md`
- [ ] `tests/` 目录
- [ ] `/health` 端点
- [ ] `/metrics` 端点
- [ ] `migrations/` 目录（无 DB 可豁免）
- [ ] Swagger 配置（`SWAGGER_ENABLED` 环境开关）

CI 门禁：`node scripts/check-service-standard.js` 全过才允许 merge。

---

## 2.5 Swagger UI 覆盖

`SWAGGER_ENABLED=true` 时业务服务暴露 Swagger UI（在每个服务的 `main.rs` 里 `SwaggerUi::new("/swagger-ui")`）。`openapi.yaml` 由 `utoipa` 注解自动生成。

**端口配置**（来自 `infrastructure/docker-compose-services.yml` + 各服务 `config.rs` / `AGENTS.md`）：

| 服务 | mesh | docker compose 外端口 | `cargo run` 本地默认端口 | Swagger URL |
|------|------|----------------------|------------------------|-------------|
| api-gateway | ✅ | 3000 | 3000（`SERVER_PORT` 默认） | http://localhost:3000/api-docs/openapi.json |
| iam-service | ✅ | 9081 | 8080（`SERVER_PORT` 默认） | http://localhost:9081/swagger-ui / http://localhost:8080/swagger-ui |
| token-service | ✅ | 9082 | 8080 | http://localhost:9082/swagger-ui / http://localhost:8080/swagger-ui |
| notification-service | ✅ | 9083 | 8080 | http://localhost:9083/swagger-ui |
| audit-service | ✅ | 9084 | 8080 | http://localhost:9084/swagger-ui |
| prue_backend | ✅ | 9087 | 8080 | http://localhost:9087/swagger-ui |
| file-service | ❌ | 8082 | 8082 | http://localhost:8082/swagger-ui |
| config-service | ❌ | 8081 | 8081 | http://localhost:8081/swagger-ui |

> **dev 模式说明**：`docker-compose.yml` 默认就包含 `docker-compose-services.yml`（含全部 sidecar）。所以**`docker compose up` 就是 mesh 模式**，不是「无 mesh 的纯本地」。要纯本地跑某个 mesh 服务，看 [01 §1.3 方式 A](./01-quick-start.md)。

> **mesh 服务的 mTLS 入站**：`consul connect proxy -listen` 默认行为见 Consul 1.15 文档；本仓库实测从同网络容器 `wget http://<svc>:908x/health` 可达（参见 `test-case/51-mtls-mesh.md`），但浏览器能否直接 `localhost:9081/swagger-ui` 取决于 sidecar 当前 TLS 模式（api-gateway sidecar 已显式 `allow_untlsed_inbound=true`，其他 sidecar 行为参考实际握手）。需要确认时 `curl -v http://localhost:9081/swagger-ui` 看响应。

**改了 `#[utoipa::path]` 注解后**重新生成：

```bash
node scripts/dump-openapi-specs.js   # 8 个服务一起
```

CI 门禁：`node scripts/check-spec.js` 校验 yaml 合法性 + `git diff --exit-code` 检测漂移。

> **绝对不要手改 `openapi.yaml`** — 它是生成产物，下次 dump 会被覆盖。

---

## 2.7 后台 worker：outbox publisher

2 个 outbox publisher worker 跑事务性发件箱模式：

| Worker | 数据源 | 目标 | 备注 |
|--------|--------|------|------|
| `iam-outbox-publisher` | `iam_outbox_events` 表 | RabbitMQ exchange `events` | poll interval 1s, batch 100 |
| `notification-outbox-publisher` | `ntf_outbox_events` 表 | RabbitMQ exchange `events` | poll interval 1s, batch 100 |

> **归档**：活行 >24h 未投递 → 移入 `*_outbox_events_archive`；30 天后 purge。

**为什么需要 outbox**：跨服务事件要在**同一个 DB 事务**里投递，避免「业务写入成功但消息丢了」或「消息发了业务没写」。模式详见 [`docs-site/ai-context.md` §7 pitfall 19](../ai-context.md)。

这些 worker **不暴露 HTTP**，不参与 mesh，只是单进程循环。

---

## 2.8 相关决策

- ADR-001 API Gateway 多文件路由配置
- ADR-002 8 项服务标准
- [Phase 5-A spec](../superpowers/specs/2026-07-03-phase5a-consul-connect-mtls-design.md) Consul Connect mTLS（已撤销；保留作历史考古，被 Phase 5-A-2 registrator 方案替代）
- 完整索引见 [`docs-site/decisions/`](../decisions/)
