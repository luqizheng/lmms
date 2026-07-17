# 07. 日志与排查 Bug

> 读完这一章你会知道：① 日志去哪看；② 怎么调日志级别；③ 一条日志怎么读；④ 怎么用 trace_id 跨服务串联一次请求；⑤ 5 个最常见的错误怎么应对。

---

## 7.1 日志去哪看

### 本地开发（cargo run）

```bash
cd services/iam-service
cargo run
```

日志直接输出到**终端 stdout**，不需要去文件里翻。

### Docker 容器

```bash
# 看 api-gateway 日志
docker logs -f coder-micro-services-api-gateway-1

# 看 MySQL 日志
docker logs -f coder-micro-services-mysql-1

# 看所有基础设施日志
docker compose -f infrastructure/docker-compose.yml logs -f
```

### 生产环境

> ⚠️ **TODO: 集中式日志收集（ELK / Loki / 阿里云 SLS）— 待运维补充**
>
> 生产环境的日志不在容器 stdout，需要补充：
> 1. 日志收集方案（用什么工具）
> 2. 查询入口 URL
> 3. 权限申请方式

---

## 7.2 调日志级别 `RUST_LOG`

Rust 的 `tracing` 库通过 `RUST_LOG` 环境变量控制日志详细程度：

| 级别 | 用途 | 输出量 |
|------|------|--------|
| `trace` | 最详细（每个函数进入/退出） | 极大 |
| `debug` | 调试信息（变量值、分支走向） | 大 |
| `info` | 正常流程关键节点 | 中等 |
| `warn` | 需要关注但不致命 | 小 |
| `error` | 错误 | 最小 |

### 设置方式

```bash
# 方式 1：临时设置（仅当前终端）
export RUST_LOG=debug
cargo run

# 方式 2：全局某个服务
cd services/iam-service
RUST_LOG=debug cargo run

# 方式 3：按模块分别设置
RUST_LOG=iam_service=debug,sqlx=warn,info cargo run
# 含义：iam_service 模块输出 debug，sqlx 模块只输出 warn，其他模块输出 info

# 方式 4：临时一次性（trace 最详细，适合定位棘手 bug）
RUST_LOG=trace,tower_http=trace cargo run
```

> 💡 日常开发推荐 `RUST_LOG=debug`，生产环境推荐 `RUST_LOG=info`。遇到诡异问题时用 `trace`。

---

## 7.3 一条日志怎么读

典型日志行：

```
2024-01-15T09:23:45.123456Z  INFO  iam_service::handlers::auth: login successful user_id=42
```

| 字段 | 含义 |
|------|------|
| `2024-01-15T09:23:45.123456Z` | 时间戳（UTC，精确到微秒） |
| `INFO` | 日志级别 |
| `iam_service::handlers::auth` | 模块路径（Rust 模块层级） |
| `login successful user_id=42` | 消息体 |

如果是 tracing 的 span 上下文，会有缩进和额外字段：

```
2024-01-15T09:23:45.123456Z  INFO  iam_service::handlers::auth: login successful user_id=42
    span: request method=POST uri=/iam/auth/login
    trace_id: 0af7651916cd43dd8448eb211c80319c
```

| 额外字段 | 含义 |
|---------|------|
| `span` | 当前所在 span（类似调用栈） |
| `trace_id` | 跨服务追踪 ID，**最重要的字段** |

---

## 7.4 跨服务串联请求（最实用）

一次 HTTP 请求可能经过 `api-gateway → iam-service → token-service`，每个服务都会产生日志。怎么把它们串起来？

### 步骤 1：拿到 trace_id

```bash
# 发请求时加 -i 看响应头
curl -i http://localhost:3000/v1/iam/health

# 在响应头中找到 X-Trace-Id
# X-Trace-Id: 0af7651916cd43dd8448eb211c80319c
```

> 💡 也可以从前端 Network 面板的 Response Headers 里找到 `X-Trace-Id`。

### 步骤 2：在 Jaeger 里看完整调用树

1. 打开 http://localhost:16686
2. 在搜索框里粘贴 `trace_id`
3. 点击搜索结果，看到完整的 span 树：

```
api-gateway (root span)
  └── iam-service
        └── token-service
```

每个 span 显示：耗时、状态码、错误信息。

### 步骤 3：在各服务日志里 grep trace_id

```bash
# 假设 trace_id=0af7651916cd43dd8448eb211c80319c
TRACE_ID="0af7651916cd43dd8448eb211c80319c"

# 在 api-gateway 日志里找
docker logs coder-micro-services-api-gateway-1 2>&1 | grep "$TRACE_ID"

# 在 iam-service 日志里找
docker logs coder-micro-services-iam-service-1 2>&1 | grep "$TRACE_ID"
```

> 本地开发时，直接在终端里 `RUST_LOG=trace cargo run | grep $TRACE_ID` 即可。

---

## 7.5 Grafana 看错误率 / 限流命中率

1. 打开 http://localhost:3001
2. 登录：`admin` / `admin`
3. 打开 "Gateway Overview" dashboard

关键面板：

| 面板名 | 看什么 |
|--------|--------|
| **Request Rate** | 每秒请求数（QPS） |
| **Error Rate** | 5xx 错误占比 |
| **Rate Limited** | 被限流的请求数 |
| **Latency (P99)** | 99% 请求的响应时间 |

> 如果 Error Rate 突然上升，点击时间范围缩小到出问题的时段，结合 trace_id 去 Jaeger 查详情。

---

## 7.6 五条常见错误样例 + 应对

### 错误 1：`connection refused mysql:3306`

**症状**：服务启动时报数据库连接失败。

**原因**：MySQL 容器没起，或者容器名变了。

**应对**：

```bash
# 1. 检查容器状态
docker ps

# 2. 如果没起，启动基础设施
docker compose -f infrastructure/docker-compose.yml up -d

# 3. 如果容器名不对（Docker Compose 版本差异），用实际名称
# docker ps 看到的 NAME 列才是准确的容器名
```

---

### 错误 2：`401 Unauthorized`

**症状**：接口返回 401。

**原因**：
- Access Token 过期（默认 15 分钟）
- Token 被加入 Redis 黑名单（用户主动登出）
- Token 签发方和服务验签的公钥不一致

**应对**：

```bash
# 1. 检查 Token 是否过期（看 JWT payload 里的 exp 字段）
# 2. 检查 Redis 黑名单
docker exec -it coder-micro-services-redis-1 redis-cli
> EXISTS blacklist:<token_jti>

# 3. 重新登录获取新 Token
curl -X POST http://localhost:3000/v1/iam/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin123"}'
```

---

### 错误 3：`404 Not Found`（但 Swagger 里有这个接口）

**症状**：Swagger UI 里能看到接口，但调用时返回 404。

**原因**：接口**存在**于服务本身，但**没注册**到 api-gateway 的路由中。

**应对**：

```bash
# 1. 检查 api-gateway 路由配置
ls infrastructure/config/api-gateway/routes/

# 2. 确认对应服务的 prefix 已配置
# 例如 iam-service 的接口前缀是 /iam/*，需要在路由文件中配置
# 编辑 infrastructure/config/api-gateway/routes/iam.router.toml

# 3. 热加载生效
curl -X POST http://localhost:3000/__admin/reload
```

---

### 错误 4：`429 Too Many Requests`

**症状**：接口返回 429，响应头带 `Retry-After: 1`。

**原因**：触发了限流。

**应对**：

```bash
# 1. 检查当前限流配置
cat infrastructure/config/api-gateway/rate-limits.toml

# 2. 如果是开发测试需要高并发，临时调高限流
# 编辑 rate-limits.toml，把对应路由的 requests_per_second 调大

# 3. 热加载
curl -X POST http://localhost:3000/__admin/reload
```

> ⚠️ 生产环境不要随意调高限流，先确认是否是正常流量。

---

### 错误 5：sqlx 编译报错 "error returned from database"

**症状**：`cargo build` 或 `cargo run` 时报数据库错误。

**原因**：sqlx 编译期需要连数据库做类型检查，但数据库 schema 和 `sqlx-data.json` 不一致。

**应对**：

```bash
# 1. 确保数据库已启动
docker compose -f infrastructure/docker-compose.yml up -d

# 2. 进入服务目录
cd services/iam-service

# 3. 重新生成 sqlx 离线数据
cargo sqlx prepare

# 4. 确保 sqlx-data.json 已提交到 git
git add sqlx-data.json
git commit -m "chore: update sqlx offline data"
```

---

## 7.7 求救前先收集这些信息

如果你排查后还是搞不定，请按以下模板提供信息，能大幅减少来回沟通：

```
【环境】本地开发 / 测试环境 / 生产环境？
【操作】你做了什么？（具体命令或操作步骤）
【期望】你期望发生什么？
【实际】实际发生了什么？（附完整报错信息）
【trace_id】X-Trace-Id 响应头值（如有）
【日志】相关服务的最近 20 行日志
【已尝试】你已经试过哪些排查步骤？
```

---

## 7.8 ⚠️ TODO：求救渠道

> ⚠️ **TODO: 团队内部求救渠道 — 待团队负责人补充**
>
> 请补充：
> 1. 技术问题 IM 群号 / 频道名
> 2. on-call 值班表与联系人
> 3. 紧急故障升级路径（谁决策、谁执行）
