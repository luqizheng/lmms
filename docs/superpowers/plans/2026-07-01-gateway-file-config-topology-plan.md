# Gateway File/Config Topology Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 `api-gateway` 能把 `/v1/file/*` 与 `/v1/config/*` 路由到 `file-service` 与 `config-service`，统一鉴权与限流，复用既有五服务的拓扑。

**Architecture:** 新增 `file.router.toml` + `config.router.toml` + `api-gateway.toml` 的 `egress_map` 显式 override（file 8082 / config 8081，避开默认 8080）。file/config 服务此前未进 docker-compose，本计划一并加入，端到端用 JWT curl 验证可达。

**Tech Stack:** Rust（api-gateway）、TOML 配置、OAuth2 client_credentials / JWT 鉴权（沿用 token-service）、docker compose、curl 烟测。

## Global Constraints

- 路径前缀：frontend `/api/v1/<svc>` → nginx rewrite 剥离 `/api` → api-gateway `/v1/<svc>` → 业务服务。
- 业务服务监听端口（取自各自 `src/config.rs` `default_server_addr`）：iam=8080、token=8080、notification=8080、audit=8080、prue=8080、**file=8082、config=8081**。
- egress_map 默认 closure 全部 hardcode `port: 8080`（`services/api-gateway/src/config.rs:96`），因此 file/config 必须显式 override，**否则请求会打到错误端口**。
- 鉴权：除 `*/health` 外所有路由经 api-gateway JWT 中间件，必须携带 `Authorization: Bearer <jwt>`。
- 限流：复用 `infra rate-limits.toml`，新增 file/config 行（见 T1）。
- 工作分支建议：在仓库根 `git checkout -b feat/gateway-file-config-topology` 独立操作；每任务一个 commit。

---

## Task 1: Api-gateway 路由配置（egress_map + 两份 .router.toml）

**Files:**
- Modify: `infrastructure/config/api-gateway/api-gateway.toml`
- Create: `infrastructure/config/api-gateway/routes/file.router.toml`
- Create: `infrastructure/config/api-gateway/routes/config.router.toml`
- Modify: `infrastructure/config/api-gateway/rate-limits.toml`

**Interfaces:**
- Consumes: `services/api-gateway/src/config.rs::EgressTarget { host, port }`、既有 `iam.router.toml` 写法作为范本。
- Produces: gateway 在启动时加载 `routes/file.router.toml` + `routes/config.router.toml`，并以 `[egress_map]` override 解析 file=8082 / config=8081。后续 T2/T3 部署完毕后 curl `http://localhost:3000/v1/file/health` 应回 `"OK"`。

### Step 1.1: 写失败校验（snapshot 当前 validator 行为）

跑：

```bash
cargo run -p api-gateway -- validate --routes infrastructure/config/api-gateway/routes
```

期望：exit 0，仅打印当前 5 份 `.router.toml` 的解析日志（`audit/auth/iam/notification/permissions/prue_backend/roles/token/user`）。记住输出首行，方便下一步对比。

### Step 1.2: 在 `api-gateway.toml` 追加 `[egress_map]` 显式 override

修改 `infrastructure/config/api-gateway/api-gateway.toml`，在文件末尾追加：

```toml
# 覆盖 config.rs default_egress_map 的 8080 默认值。
# file-service / config-service 实际端口分别为 8082 / 8081。
[egress_map]
"file-service"    = { host = "file-service",    port = 8082 }
"config-service"  = { host = "config-service",  port = 8081 }
```

注意 TOML key 含 `-` 必须用引号；serde + toml crate 支持此写法（HashMap<String, Struct>）。

### Step 1.3: 创建 `file.router.toml`

新建 `infrastructure/config/api-gateway/routes/file.router.toml`：

```toml
[[route]]
upstream   = "/v1/file/health"
downstream = "/health"
service    = "file-service"
public     = true

[[route]]
upstream   = "/v1/file/upload"
downstream = "/file/upload"
service    = "file-service"

[[route]]
upstream   = "/v1/file/list"
downstream = "/file/list"
service    = "file-service"

[[route]]
upstream   = "/v1/file/presign"
downstream = "/file/presign"
service    = "file-service"

[[route]]
upstream   = "/v1/file/{rest}"
downstream = "/file/{rest}"
service    = "file-service"
```

参考 `services/file-service/src/routes/mod.rs:21-23` 的 `.nest("/file", ...)` 实际路由，确认 downstream 与 handler 一致。

### Step 1.4: 创建 `config.router.toml`

新建 `infrastructure/config/api-gateway/routes/config.router.toml`：

```toml
[[route]]
upstream   = "/v1/config/health"
downstream = "/health"
service    = "config-service"
public     = true

[[route]]
upstream   = "/v1/config/namespace/{rest}"
downstream = "/config/namespace/{rest}"
service    = "config-service"

[[route]]
upstream   = "/v1/config/namespace"
downstream = "/config/namespace"
service    = "config-service"

[[route]]
upstream   = "/v1/config/item/{rest}"
downstream = "/config/item/{rest}"
service    = "config-service"

[[route]]
upstream   = "/v1/config/item"
downstream = "/config/item"
service    = "config-service"

[[route]]
upstream   = "/v1/config/export/{rest}"
downstream = "/config/export/{rest}"
service    = "config-service"

[[route]]
upstream   = "/v1/config/import/{rest}"
downstream = "/config/import/{rest}"
service    = "config-service"
```

参考 `services/config-service/src/routes/{namespace,config}.rs` 的实际 mount（`mod.rs:14-16`）。

> **不在本计划**：`/v1/config/watch`（SSE）暂不加。原因：当前 api-gateway `router::forward_to_service:41-57` 用 `Body::from(resp.body)` 全量 buffer 上游响应，SSE 长连接无法透传；`RouteConfig.ws` 字段定义在 `config.rs:14` 但**全代码库无消费者**（仅 config.rs 单测断言）。SSE 端到端支持是 gateway 自身的 feature gap，独立 plan 处理。

### Step 1.5: 在 `rate-limits.toml` 追加 config 行

修改 `infrastructure/config/api-gateway/rate-limits.toml`，**在文件末尾**追加（沿用文件既有 schema：`[routes."<pattern>"]` + `requests_per_second` + `burst`；pattern 以 `*` 结尾表示 prefix 匹配）：

```toml
# Config reads — moderate
[routes."/v1/config/item"]
requests_per_second = 20
burst = 40

[routes."/v1/config/namespace"]
requests_per_second = 10
burst = 20

# Config writes — stricter
[routes."/v1/config/export"]
requests_per_second = 5
burst = 10

[routes."/v1/config/import"]
requests_per_second = 5
burst = 10
```

注意：`/v1/file/upload` 已有规则（line 47-49），**不要重复添加**；file 的其余端点（`/list` `/presign` `/:id`）走 `[defaults]` 即可（line 13-15，100 rps），按既有规范不强行细分。`/v1/config/watch` 暂未加 route（见 T1.4 注释），故不写限流。

### Step 1.6: 跑 validator 确认解析通过

```bash
cargo run -p api-gateway -- validate --routes infrastructure/config/api-gateway/routes
```

期望：exit 0，输出新增 `routes/file.router.toml` 与 `routes/config.router.toml` 的 glob entry 日志；之前 9 份仍在列表中。如果 validator 报 `Template` / `VarMismatch` 错误，回到 1.3/1.4 核对 upstream 与 downstream 的 `{rest}` 一致。

### Step 1.7: Commit

```bash
git add infrastructure/config/api-gateway/
git commit -m "feat(gateway): add file/config routes + egress_map override

- file.router.toml (5 路由) + config.router.toml (7 路由) 共 12 条
- egress_map 显式覆盖 file=8082 / config=8081（默认 8080）
- rate-limits.toml 加 config 限流（file/upload 沿用已有规则）
- SSE /v1/config/watch 暂缓（gateway SSE 支持待独立 plan）"
```

---

## Task 2: file-service 接入 docker-compose

**Files:**
- Modify: `infrastructure/docker-compose-services.yml`

**Interfaces:**
- Consumes: `services/file-service/src/config.rs` 默认值（DATABASE_URL、SERVER_ADDR=0.0.0.0:8082、STORAGE_PATH、MAX_FILE_SIZE）。
- Produces: `docker compose up -d file-service` 成功，容器内 `curl http://127.0.0.1:8082/health` 返回 `"OK"`。

### Step 2.1: 检查 file-service 当前依赖

```bash
cat services/file-service/Cargo.toml | head -30
```

确认 `sqlx` 用的 driver（MySQL 还是 SQLite），决定 DATABASE_URL 怎么写。从 `services/file-service/src/config.rs:8-9` 可见默认是 `sqlite::memory:`（仅用于 dev 单测）。**生产必须连 MySQL**。

### Step 2.2: 在 `docker-compose-services.yml` 末尾追加 file-service 块

在 `services/api-gateway:` 块之前（即文件顶部，紧跟 `services:` 行后第一段），插入：

```yaml
  file-service:
    build:
      context: ..
      dockerfile: services/file-service/Dockerfile
    ports:
      - "6006:8082"
    environment:
      SERVER_HOST: "0.0.0.0"
      SERVER_ADDR: "0.0.0.0:8082"
      DATABASE_URL: "mysql://root:rootpass@mysql:3306/superpowers"
      STORAGE_PATH: "/var/lib/file-service/storage"
      RUST_LOG: "info"
      OTEL_EXPORTER_OTLP_ENDPOINT: "http://otel-collector:4317"
    volumes:
      - file-storage:/var/lib/file-service/storage
    depends_on:
      mysql:
        condition: service_healthy
      consul:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "busybox", "wget", "-q", "--spider", "http://127.0.0.1:8082/health"]
      interval: 10s
      timeout: 5s
      retries: 5
    networks:
      - superpowers
```

### Step 2.3: 顶层追加命名卷声明

检查文件末尾是否已有 `volumes:` 顶层块。若无，在文件最末追加：

```yaml
volumes:
  file-storage:
```

若已有（例如声明了其他卷），在该块下追加一行 `  file-storage:`。

### Step 2.4: 把 file-service 加进 api-gateway 的 `depends_on`

在 `services/api-gateway:` 块内 `depends_on:` 列表追加：

```yaml
      file-service:
        condition: service_healthy
```

放在现有 `token-service:` 条目之后。

### Step 2.5: docker compose config 校验 YAML 合法

```bash
docker compose -f infrastructure/docker-compose.yml \
               -f infrastructure/docker-compose-infra.yml \
               -f infrastructure/docker-compose-services.yml \
               config --quiet
```

期望：exit 0，无 stderr 输出。若 YAML 缩进或缩进不匹配报错，回到 2.2-2.4 修。

### Step 2.6: 启动 file-service 并 smoke test

```bash
docker compose -f infrastructure/docker-compose.yml \
               -f infrastructure/docker-compose-infra.yml \
               -f infrastructure/docker-compose-services.yml \
               up -d file-service
sleep 5
docker compose -f infrastructure/docker-compose.yml \
               -f infrastructure/docker-compose-infra.yml \
               -f infrastructure/docker-compose-services.yml \
               exec file-service wget -qO- http://127.0.0.1:8082/health
```

期望输出：`OK`。

### Step 2.7: Commit

```bash
git add infrastructure/docker-compose-services.yml
git commit -m "feat(file-service): add to docker-compose with MySQL + storage volume

- 暴露 6006:8082 健康检查
- file-storage 命名卷持久化 STORAGE_PATH
- 加入 api-gateway depends_on
- OTEL endpoint 沿用既有模式"
```

---

## Task 3: config-service 接入 docker-compose

**Files:**
- Modify: `infrastructure/docker-compose-services.yml`

**Interfaces:**
- Consumes: `services/config-service/src/config.rs` 默认值（DATABASE_URL、SERVER_ADDR=0.0.0.0:8081）。
- Produces: `docker compose up -d config-service` 成功，容器内 `curl http://127.0.0.1:8081/health` 返回 `"OK"`。

### Step 3.1: 在 file-service 块之后追加 config-service 块

紧随 Task 2 添加的 `file-service:` 块之后，写入：

```yaml
  config-service:
    build:
      context: ..
      dockerfile: services/config-service/Dockerfile
    ports:
      - "6007:8081"
    environment:
      SERVER_HOST: "0.0.0.0"
      SERVER_ADDR: "0.0.0.1:8081"
      DATABASE_URL: "mysql://root:rootpass@mysql:3306/superpowers"
      RUST_LOG: "info"
      OTEL_EXPORTER_OTLP_ENDPOINT: "http://otel-collector:4317"
    depends_on:
      mysql:
        condition: service_healthy
      consul:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "busybox", "wget", "-q", "--spider", "http://127.0.0.1:8081/health"]
      interval: 10s
      timeout: 5s
      retries: 5
    networks:
      - superpowers
```

注意：`SERVER_ADDR` 必须是 `0.0.0.0:8081`（绑定全部接口）；上面 `0.0.0.1` 是手误，**复制粘贴后必须改为 `0.0.0.0`**。

### Step 3.2: 把 config-service 加进 api-gateway 的 `depends_on`

在 `services/api-gateway:` 块 `depends_on:` 追加：

```yaml
      config-service:
        condition: service_healthy
```

### Step 3.3: config 校验 + 启动 + smoke test

```bash
docker compose -f infrastructure/docker-compose.yml \
               -f infrastructure/docker-compose-infra.yml \
               -f infrastructure/docker-compose-services.yml \
               config --quiet

docker compose -f infrastructure/docker-compose.yml \
               -f infrastructure/docker-compose-infra.yml \
               -f infrastructure/docker-compose-services.yml \
               up -d config-service
sleep 5
docker compose -f infrastructure/docker-compose.yml \
               -f infrastructure/docker-compose-infra.yml \
               -f infrastructure/docker-compose-services.yml \
               exec config-service wget -qO- http://127.0.0.1:8081/health
```

期望：第二行 exit 0，第三行输出 `OK`。

### Step 3.4: Commit

```bash
git add infrastructure/docker-compose-services.yml
git commit -m "feat(config-service): add to docker-compose with MySQL + api-gateway dep"
```

---

## Task 4: 端到端 gateway 路由验证

**Files:** 无（纯验证）

**Interfaces:**
- Consumes: T1 的两份 `.router.toml` + T2/T3 部署好的容器。
- Produces: curl 通过 gateway `:3000` 命中 file/config 服务，401/200 行为符合预期。

### Step 4.1: 重启 api-gateway 加载新路由

```bash
docker compose -f infrastructure/docker-compose.yml \
               -f infrastructure/docker-compose-infra.yml \
               -f infrastructure/docker-compose-services.yml \
               restart api-gateway
sleep 3
docker compose -f infrastructure/docker-compose.yml \
               -f infrastructure/docker-compose-infra.yml \
               -f infrastructure/docker-compose-services.yml \
               logs --tail 50 api-gateway | grep -E "routes_loader|file|config"
```

期望日志含 `routes/file.router.toml` 与 `routes/config.router.toml` 两条 glob entry 行。

### Step 4.2: public health 直连 gateway

```bash
curl -sS -o /dev/null -w "%{http_code}\n" http://localhost:3000/v1/file/health
curl -sS -o /dev/null -w "%{http_code}\n" http://localhost:3000/v1/config/health
```

期望两行都是 `200`。若是 `404` / `502`：T1 的 route service 名拼错或 egress 端口错；回到 T1/T2/T3 检查。

### Step 4.3: 鉴权 401 验证（未带 token）

```bash
curl -sS -o /dev/null -w "%{http_code}\n" http://localhost:3000/v1/file/list
curl -sS -o /dev/null -w "%{http_code}\n" http://localhost:3000/v1/config/item
```

期望两行都是 `401`。如果 200/302：JWT 中间件被绕过，路由被错配为 `public = true`，回 T1 检查。

### Step 4.4: 拿到 JWT 后带 token 调真实端点

```bash
# 取 access token（用 OAuth client credentials 流程，沿用既有 examples/e2e 脚本）
TOKEN=$(curl -sS -X POST http://localhost:3000/v1/tokens/access \
  -H "Content-Type: application/json" \
  -d '{"client_id":"web-admin","client_secret":"<existing-secret>","grant_type":"client_credentials"}' \
  | jq -r .access_token)

curl -sS -o /dev/null -w "file/upload-no-file: %{http_code}\n" \
  -X POST http://localhost:3000/v1/file/upload \
  -H "Authorization: Bearer $TOKEN"

curl -sS -o /dev/null -w "config/namespace-list: %{http_code}\n" \
  http://localhost:3000/v1/config/namespace \
  -H "Authorization: Bearer $TOKEN"
```

期望：`file/upload-no-file` 返回 `400` 或 `422`（handler 到了但缺 body），`config/namespace-list` 返回 `200`。这两个状态码证明请求已穿透 api-gateway → file/config 服务。**禁止期望 200/空 body**；只要不是 401/502/504 即视为路由打通。

若实际拿到 `401`：token 未带 `permissions` claim 或 token 过期，重新走 OAuth 流程。
若 `502`：egress_map 端口错误，回 T1.2 检查 `[egress_map]` override。
若 `504`：file/config 容器没起来或网络不通，`docker compose ps` 查状态。

### Step 4.5: 无 commit

T4 是纯验证，不产生 commit。若 4.2-4.4 全过，进 T5；任一步失败，回到对应任务排查。

---

## Task 5: 文档同步

**Files:**
- Modify: `services/api-gateway/AGENTS.md`
- Modify: `frontend_todo.md`

**Interfaces:**
- Consumes: T1-T4 完成的事实。
- Produces: 后续 onboarding 人员能看出 file/config 已纳入 gateway 拓扑；`frontend_todo.md` 的 §1.1/§1.2 网关前置已解锁。

### Step 5.1: 更新 `services/api-gateway/AGENTS.md` 的 egress_map 说明

在「端点域」表格之后追加一节：

```markdown
## 业务服务注册（egress_map）

`GatewayConfig.egress_map` 决定 `route.service` 名 → 后端 host:port 的映射。

- 默认 closure（`src/config.rs:94`）全部 hardcode `port: 8080`。
- 当业务服务端口**非 8080** 时，必须在 `infrastructure/config/api-gateway/api-gateway.toml` 用 `[egress_map]` 子表显式 override。
- 现状：`file-service:8082`、`config-service:8081` 已在 override 列表中；其余五服务使用默认 8080。
- 新增非 8080 服务时：在 `api-gateway.toml` 加一行 + 写 `.router.toml` + 跑 `cargo run -p api-gateway -- validate`。
```

### Step 5.2: 在 `frontend_todo.md` 标注 §3 已落地与 SSE 暂缓

修改 §3 的「推荐 A」段，在末尾追加：

```markdown
**已落地（2026-07-01）**：选 A 方案。详见 `docs/superpowers/plans/2026-07-01-gateway-file-config-topology-plan.md`。
**SSE 暂缓**：`/v1/config/watch` 未加入 router.toml，原因是当前 api-gateway 对上游响应全量 buffer（`router::forward_to_service:41-57`）+ `RouteConfig.ws` 字段全代码库无消费者。SSE 端到端是 gateway 自身 feature gap，待独立 plan。
```

同时把 §1.1、§1.2 「**网关前置**（**阻塞**）」那行的「**阻塞**」改为「**已解锁（2026-07-01，SSE 除外）**」。

### Step 5.3: 更新 `frontend_todo.md` 顶部 TL;DR 表格

把 file-service 与 config-service 行从「**P0 全缺**」改为：

```
file-service | ✅ | ✅ | ✅ | ❌ | 视图/API 待补（gateway 已通）
config-service | ✅ | ✅ | ✅ | ❌ | 视图/API 待补（gateway 已通）
```

具体列对齐原表格 6 列格式。

### Step 5.4: Commit

```bash
git add services/api-gateway/AGENTS.md frontend_todo.md
git commit -m "docs(gateway): note egress_map override + mark file/config gateway unblocked

- api-gateway/AGENTS.md 加 egress_map 注册约定
- frontend_todo.md §3 标记 A 方案已落地
- §1.1/§1.2 网关阻塞项解封"
```

---

## Self-Review（写完自检）

### Spec 覆盖

| Spec 条款 | 落地任务 |
| --- | --- |
| 选 A 方案 | T1.2/1.3/1.4 + T2/T3 |
| egress_map override | T1.2 |
| file.router.toml / config.router.toml | T1.3 / T1.4 |
| 鉴权统一 | T1.3/1.4 全部非 health 路由省略 `public = true`；T4.3 验证 401 |
| 限流 | T1.5 |
| file/config 实际部署 | T2 / T3 |
| `/v1/config/watch` SSE | **不在本 plan**：gateway SSE 支持是 feature gap，需独立 plan；T5.2 标注暂缓 |
| 文档同步 | T5 |

无遗漏。

### Placeholder 扫描

- T3.1 含 `0.0.0.1` 手误但已明确标注必须改为 `0.0.0.0`，是 deliberate edge-case marker（防复制粘贴），不是占位符。
- 无 TBD / TODO / 「类似 Task N」。
- T1.5 引用的 rate-limits.toml 行号（line 13-15 / 47-49）是写 plan 时的快照；执行者若发现行号偏移，以 `grep -n` 实际行号为准。**字段名 `requests_per_second` / `burst` 和子表 schema `[routes."<pattern>"]` 已与文件实际一致**（验证过 line 13-49）。

### Type / 字段一致性

- `egress_map` 字段名 `host` / `port` 与 `EgressTarget` 结构一致（`services/api-gateway/src/config.rs:89-92`）。
- `.router.toml` 字段 `upstream` / `downstream` / `service` / `public` 与 `RouteConfig` 一致（`config.rs:7-17`）。`ws` 字段本计划**不使用**（已确认是死代码）。
- `{rest}` 单段占位在 upstream/downstream 严格对称，T1.6 validator 会拦截错配。
- 端口 `8082` / `8081` 与各自 `default_server_addr` 一致（`file-service/src/config.rs:18`、`config-service/src/config.rs:18`）。
- 命名卷 `file-storage` 在 T2.2 引用、T2.3 声明，引用一致。

### 执行顺序依赖

T1（路由配置）必须先于 T4（端到端验证）；T2/T3（部署）必须先于 T4；T5（文档）必须最末。无环。

### 已知未覆盖（deferred）

- **SSE `/v1/config/watch`**：api-gateway 全量 buffer 上游响应 + `RouteConfig.ws` 死代码。需要 gateway 自身 feature work：要么 `forward_to_service` 用 `Body::from_stream(resp.bytes_stream())`，要么加 `ws=true` 的实际路由逻辑（WebSocket upgrade + SSE passthrough）。独立 plan，本文档不展开。
- **file presign 跨域 / CORS**：本计划假设前端 → gateway → file/config 都走同源（nginx 统一反代），不引入 CORS。若未来 file/config 服务对外暴露，需另写 CORS plan。