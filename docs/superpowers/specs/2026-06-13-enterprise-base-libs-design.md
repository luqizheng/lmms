# 企业级基础微服务与共享内核设计

- 日期：2026-06-13
- 范围：仓库 `coder-micro-services`
- 形态：双端基础库（Rust 后端基础微服务 + 前端基础库 + 双端契约层）
- 交付：总体设计文档（本文件）

---

## 1. 背景与目标

为企业级开发提供「可直接接入」的基础微服务与共享内核，覆盖身份权限、配置、审计、通知、文件五大能力域，并以 libs 共享内核保障跨服务一致性。

非目标：

- 不引入消息队列（MQ）。
- 不重写现有 `api-gateway`。
- 不为 C 端产品做前端能力（性能埋点、SEO 等）。

主要服务场景：后台管理系统 / 微服务运行时 / 开发者工具。

---

## 2. 总体架构

### 2.1 服务拓扑

```
coder-micro-services/
├─ services/
│  ├─ api-gateway/             (已存在)
│  ├─ iam-service/             【新】身份/用户/角色/权限/菜单/部门/Token
│  ├─ config-service/          【新】配置中心（多环境/版本/灰度）
│  ├─ audit-service/           【新】操作日志 + 审计 + 登录日志
│  ├─ notification-service/    (已存在，扩展) 站内信/邮件/Webhook
│  ├─ file-service/            【新】基于 rustfs 的文件上传/下载/预签名
│  ├─ token-service/           (内核保留，能力并入 iam-service)
│  ├─ credential-login-service/(stub → 并入 iam-service)
│  └─ user-service/            (并入 iam-service)
├─ libs/                       共享内核（Rust）
├─ contracts/                  双端契约（YAML + 代码生成）
└─ frontend-libs/              前端基础库（pnpm workspace，可选阶段）
```

### 2.2 libs 共享内核

```
libs/
├─ jwt-ctx/        (已存在)
├─ digua_authz/          (已存在)
├─ digua_common-error/   统一错误 + HTTP 映射
├─ digua_common-config/  env+toml+consul+config-service 加载
├─ digua_common-log/     tracing + request_id + 审计上报
├─ digua_common-http/    reqwest 封装 + 重试 + 服务发现
├─ digua_common-db/      sqlx + 多方言（mysql/dameng/mssql/sqlite）
├─ digua_common-cache/   Redis + 本地双层 + 分布式锁
├─ digua_common-crypto/  AES/RSA/Argon2/HMAC
└─ digua_common-validator/ 入参校验扩展
```

### 2.3 内部依赖图（避免环）

```
digua_common-error  ← 所有
digua_common-log    ← 仅依赖 digua_common-error
digua_common-config ← 依赖 error/log
digua_common-db     ← 依赖 error/log/config
digua_common-cache  ← 依赖 error/log/config
digua_common-http   ← 依赖 error/log/config
digua_common-crypto ← 仅依赖 error
digua_common-validator ← 仅依赖 error
jwt-ctx / digua_authz ← 依赖 error/log/cache
```

---

## 3. 多数据库支持（digua_common-db 重点）

### 3.1 支持矩阵

| 数据库 | 驱动 | 方言键 | 用途 |
|---|---|---|---|
| MySQL | sqlx-mysql | `mysql` | 生产 |
| 达梦 | sqlx-mysql（DM MySQL 兼容模式） | `dameng` | 信创生产 |
| MSSQL | sqlx-mssql 或 tiberius | `mssql` | 生产 |
| SQLite | sqlx-sqlite | `sqlite` | 单测专用 |

### 3.2 模块结构

```
libs/digua_common-db/
├─ src/
│  ├─ lib.rs              # DbPool 枚举、init_pool()
│  ├─ dialect/
│  │  ├─ mod.rs           # trait Dialect
│  │  ├─ mysql.rs
│  │  ├─ dameng.rs        # 继承 mysql，覆盖关键字/SEQ/分页差异
│  │  ├─ mssql.rs         # OFFSET FETCH / TOP / IDENT
│  │  └─ sqlite.rs        # AUTOINCREMENT / 无 RIGHT JOIN
│  ├─ migration/runner.rs
│  ├─ tx.rs
│  └─ test_kit/
│     ├─ sqlite_fixture.rs   # 临时文件 + tx 回滚
│     └─ parallel.rs
└─ Cargo.toml             # feature: mysql / dameng / mssql / sqlite
```

### 3.3 核心契约

- `Dialect` trait 抽象差异：分页、UPSERT、当前时间、JSON 字段、布尔、UUID。
- 业务代码仅依赖 `&dyn Repository`，不直接写方言 SQL。
- 迁移目录：`migrations/<service>/<dialect>/V001__init.sql`。
- 服务从 `DATABASE_URL` 自动识别方言：`mysql://` / `dameng://` / `mssql://` / `sqlite://`。

### 3.4 SQLite 单测策略

- 每用例独立 `tempfile::NamedTempFile`，并行安全。
- 启动迁移用 `migrations/<service>/sqlite/`。
- 外层包事务，drop 即回滚。
- 失败时输出 tempfile 路径供调试。

### 3.5 方言兼容矩阵

CI 用 docker 起 dameng / mssql 各跑一次核心 service 测试，频次每日定时（不阻塞每次 PR）。

---

## 4. 五大基础微服务能力清单

### 4.1 iam-service（身份与权限中心）

端点域：`/iam/*`

- 认证：登录（密码 / 验证码 / SSO）、登出、刷新 Token、改密、找回密码。
- 用户：CRUD、批量导入、状态、头像、最后登录。
- 角色：CRUD、角色-权限绑定、数据范围。
- 权限：菜单 / 按钮 / API 权限，与 `permissions.yaml` 同步。
- 菜单：树形 CRUD、按角色返回前端菜单。
- 部门：树形 CRUD、用户-部门 N:M。
- Token：JWT 签发 / 校验 / 吊销，复用现有 token-service 内核。
- 在线会话：在线用户列表、强制下线。

依赖：digua_common-db / digua_common-cache / digua_common-crypto / digua_common-log。

### 4.2 config-service（配置中心）

端点域：`/config/*`

- 命名空间：app + env（dev/staging/prod）+ profile。
- 配置项：key/value、类型（string/int/bool/json/yaml）、版本、变更记录。
- 灰度发布：按实例标签 / IP 段下发。
- 监听：长轮询 / SSE 推送，客户端走 `digua_common-config` SDK。
- 加密项：标记为 secret 的值用 KMS-key 加密落库。
- 导入导出：toml / yaml / json。
- 与 Consul KV 边界：config-service 为业务配置层；Consul 仅服务发现。

### 4.3 audit-service（日志与审计）

端点域：`/audit/*`

- 操作日志：业务方通过 `digua_common-log` 上报，按用户 / 资源 / 动作检索。
- 登录日志：成功 / 失败、IP、UA、地理位置。
- 数据变更：before / after diff。
- API 访问日志：网关旁路上报，采样可配。
- 合规导出：CSV / Excel。
- 保留策略：hot 90 天 / warm 1 年 / cold 归档。
- 告警规则：异常登录、敏感操作触发通知。
- 上报通道：HTTP 同步 + 本地 buffer 重放（无 MQ）。

### 4.4 notification-service（通知中心，扩展）

端点域：`/notify/*`

- 渠道：站内信、邮件（SMTP）、短信（阿里 / 腾讯 trait）、Webhook。
- 模板：变量插值、多语言、版本管理。
- 发送：单发 / 批量 / 定时（内置 scheduler）/ 按角色或部门定向。
- 站内信：收件箱、已读未读、归档。
- 偏好：用户级渠道开关。
- 重试：指数退避，失败入死信表，可手动重投。
- 统计：发送量、成功率、模板维度。

### 4.5 file-service（文件服务，rustfs 后端）

端点域：`/file/*`

- 上传：单文件 / 分片 / 断点续传。
- 下载：直链 / 防盗链 / 限速。
- 预签名：put / get URL，TTL 可配。
- 元数据：业务标签、归属租户 / 用户、MIME、哈希、扫描钩子。
- 权限：与 iam 权限码联动，按文件 ACL 校验。
- 生命周期：临时文件 TTL 清理、归档迁移。
- 缩略图 / 转码：图片预设规格生成（可选异步）。
- 后端抽象：`ObjectStore` trait，默认 rustfs，备选 S3 兼容。

---

## 5. libs 共享内核详表

### 5.1 digua_common-error

- `enum AppError`：thiserror，分类 BadRequest / Unauthorized / Forbidden / NotFound / Conflict / Internal / External。
- `From<sqlx::Error>` / `From<reqwest::Error>` / `From<validator::ValidationErrors>` 自动转换。
- `IntoResponse`（axum）：输出 `{ code, message, request_id }`。
- 错误码常量来自 `contracts/error-codes.yaml` 代码生成。

### 5.2 digua_common-config

- 加载顺序：默认值 → `config/<env>.toml` → 环境变量 → consul KV → config-service。
- 类型化：`#[derive(Deserialize)] struct AppConfig`，启动期校验失败 panic。
- 热更新：订阅 config-service 变更，触发字段级 callback。
- 敏感字段：`#[secret]` 标记，日志脱敏。

### 5.3 digua_common-log

- 全局 subscriber：JSON 格式，级别可配，采样率可配。
- middleware：注入 `request_id` / `trace_id`，从 header `X-Request-Id` 透传。
- 跨服务传播：reqwest 客户端自动携带。
- 审计 API：`audit::log(action, resource, before, after)` 异步推 audit-service，本地 buffer 落盘兜底。

### 5.4 digua_common-http

- reqwest 封装：连接池、超时、TLS。
- 中间件：重试（指数退避 + 抖动）、熔断（半开态）、限流（令牌桶）。
- 自动透传 `request_id` / `trace_id` / `Authorization`。
- 服务发现：从 Consul 解析 `service-name` → 实例列表 + 轮询。
- mock：测试模式可注入 stub。

### 5.5 digua_common-db

见 §3。

### 5.6 digua_common-cache

- trait `Cache`：`get / set / del / incr / mget / lock`。
- 实现：Redis（生产）、Moka（本地 LRU）、Memory（测试）。
- 双层模式：本地 + Redis，pub/sub 广播失效。
- 分布式锁：Redis SETNX + Lua 解锁。
- key 命名：`<service>:<entity>:<id>`，TTL 强制设置。

### 5.7 digua_common-crypto

- 对称：AES-256-GCM。
- 非对称：RSA-OAEP / Ed25519。
- 哈希：SHA-256 / Blake3。
- 密码：Argon2id（默认参数）。
- HMAC：SHA-256 / SHA-512。
- KMS 抽象：trait `KeyProvider`，本地密钥 + 阿里 / 华为 KMS（可选）。
- crypto provider 锁定 `aws_lc_rs`（项目规则）。

### 5.8 digua_common-validator

- 基于 `validator` crate 扩展：手机号、身份证、统一社会信用代码、强密码。
- 自定义错误码映射 `digua_common-error`。

### 5.9 jwt-ctx / digua_authz

- 已存在；与 iam-service 的 `permissions.yaml` 同步。

---

## 6. 契约层（contracts/）

### 6.1 目录

```
contracts/
├─ error-codes.yaml      # 全局错误码字典
├─ permissions.yaml      # 权限码字典
├─ events.yaml           # 领域事件名（预留）
├─ http-conventions.md   # 响应/分页/排序约定
└─ codegen/
   ├─ gen-rust.js        # → libs/digua_common-error/src/codes.rs
   ├─ gen-rust-perms.js  # → libs/digua_authz/src/perms.rs
   └─ gen-ts.js          # → frontend/src/contracts/*.ts
```

### 6.2 规则

- 错误码：`<SERVICE>_<CATEGORY>_<NUMBER>`，例 `IAM_AUTH_1001`、`CFG_VALID_2003`。
- 字段命名：snake_case；时间字段统一 `*_at` ISO8601 UTC；ID 用 `String`（雪花 i64 转 string）。
- HTTP 响应：`{ code, message, data, request_id }`。
- 分页：`{ items, total, page, size }`。
- 生成时机：`node scripts/contracts-gen.js`，CI 校验未漂移。

---

## 7. 测试策略

| 层 | 目标 | 数据库 | 工具 |
|---|---|---|---|
| unit | libs / 单服务模块 | SQLite 临时文件 + tx 回滚 | cargo test |
| service | 单服务集成 | SQLite | tests/specs/*.yaml → 生成 |
| e2e | 跨服务（gateway 路由） | docker compose（mysql） | test-orchestrator.js |
| 兼容性 | 方言验证 | mysql / dameng / mssql 三跑 | CI matrix（每日） |

SQLite 并行单测：每用例独立 tempfile，tx 回滚，失败保留路径。

---

## 8. 演进路线

### Phase 1 — 内核就位

- libs：digua_common-error / digua_common-log / digua_common-config / digua_common-db / digua_common-cache。
- contracts：error-codes / permissions + 代码生成器。
- iam-service：合并现有 token + credential + user 三服务。

### Phase 2 — 业务基础服务

- audit-service。
- config-service（含客户端 SDK 推送）。
- file-service（rustfs 集成）。

### Phase 3 — 完善

- notification-service 扩展（邮件 / SMS / Webhook）。
- digua_common-http / digua_common-crypto / digua_common-validator 完整化。
- 方言兼容矩阵（dameng / mssql 实测）。

### Phase 4 — 加固

- 性能压测、熔断 / 限流默认开。
- 审计合规导出、保留策略。
- 前端 SDK 自动生成（OpenAPI 路线，可选）。

---

## 9. 兼容性 / 性能 / 易用性

兼容性：

- 方言适配在 `digua_common-db` 闭环，业务代码方言无关。
- 错误码 / 权限码契约升级走 codegen，CI 防漂移。
- libs feature flag 按需启用（`mysql / dameng / mssql / sqlite / redis / consul`）。

性能：

- 全异步（tokio + axum）。
- 连接池统一管理（sqlx pool + reqwest pool）。
- 双层缓存默认开。
- tracing 采样率运行时可调。
- 审计 / 日志走异步 channel + 本地 buffer，不阻塞业务。

易用性：

- 服务模板：`cargo generate` 起新服务即得日志 / 配置 / DB / HTTP / 错误处理。
- 命令统一：`node scripts/test-orchestrator.js`、路由 `validate`、`contracts-gen`。
- 文档统一在 `docs/superpowers/specs/`。

---

## 10. 风险与未决项

- 达梦 MySQL 兼容模式存在边界差异（关键字、JSON、SEQ），需在 `digua_common-db/dialect/dameng.rs` 显式覆盖。
- MSSQL 选 `sqlx-mssql` 还是 `tiberius`：待 Phase 3 实测决定。
- iam-service 合并三个现有服务需要数据迁移方案（Phase 1 子计划）。
- 审计高写入场景下 SQLite 单测可能与生产差异较大，需补充 mysql 集成测试。
- rustfs 接入接口稳定性需在 file-service 启动前评估。

---

## 11. 后续动作

1. 用户 review 本设计。
2. 确认后进入 `writing-plans`，对 Phase 1 产出实施计划。
