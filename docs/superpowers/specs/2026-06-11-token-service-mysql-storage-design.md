# token-service 切换到 MySQL 存储 — 设计

> 日期：2026-06-11
> 范围：`services/token-service/`
> 关联：`services/user-service/`（db 模式参考）

## 1. 目标与边界

### 目标
`token-service` 去掉 Redis 与 `clients.json` 文件存储，全部运行时状态与配置改用 MySQL，对外 HTTP API、JWT 签发、JWKS 端点行为完全不变。

### 范围
- ✅ Redis 中 3 类数据迁 MySQL：`auth_codes`（授权码）/ `revoked_jtis`（黑名单）/ `refresh_markers`（refresh family 标记）
- ✅ `clients.json` 文件迁 MySQL（`oauth_clients` 表）
- ✅ 对外 HTTP 接口、请求/响应体、JWT 格式、JWKS 输出保持不变
- ❌ 不迁移存量数据
- ❌ 不动 `issuer.rs`（JWT 签发逻辑）
- ❌ 不动 `consul.rs`、Consul 注册
- ❌ 不改 routes 路径

### 不在范围
- PostgreSQL / SQLite 支持（保留 `DbType` 枚举便于未来扩展，本期只出 `Mysql` 变体）
- api-gateway、user-service、notification-service 内部代码

## 2. 架构

### 分层
```
routes/  →  service/  →  store/mysql  →  MySQL
  薄处理     业务规则     sqlx 原生 SQL
```

### State 改造
`TokenServerState` 字段调整：

| 旧字段 | 新字段 |
|---|---|
| `redis_conn: MultiplexedConnection` | `db: DbPool` |
| `redis_prefix: String` | （删除） |

`DbPool` 枚举与 user-service 同形：

```rust
#[derive(Clone)]
pub enum DbPool {
    Mysql(MySqlPool),
}
```

`db::init_pool` 额外执行：
```rust
sqlx::query("SET time_zone = '+00:00'").execute(&pool).await?;
```
通过 `MySqlPoolOptions::after_connect` 钩子注册，确保每个连接都强制 UTC session。

## 3. 表结构

文件：`services/token-service/migrations/20260611000001_init.sql`

```sql
-- 1) OAuth 客户端注册表
CREATE TABLE oauth_clients (
  client_id             VARCHAR(64)  PRIMARY KEY,
  client_secret_hash    VARCHAR(255) NOT NULL,
  name                  VARCHAR(128) NOT NULL,
  permissions           JSON         NOT NULL,
  grant_types           JSON         NOT NULL,
  access_token_ttl_sec  BIGINT       NOT NULL,
  refresh_token_ttl_sec BIGINT       NOT NULL,
  status                VARCHAR(16)  NOT NULL DEFAULT 'active',
  created_at            TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at            TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

-- 2) 授权码（替换 SET_EX prefix:code → user_id）
CREATE TABLE auth_codes (
  code        VARCHAR(128) PRIMARY KEY,
  user_id     VARCHAR(64)  NOT NULL,
  expires_at  TIMESTAMP    NOT NULL,
  consumed_at TIMESTAMP    NULL,
  created_at  TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  INDEX idx_auth_codes_expires (expires_at)
);

-- 3) 吊销名单（替换 SET_EX prefix:blacklist:jti）
CREATE TABLE revoked_jtis (
  jti        VARCHAR(128) PRIMARY KEY,
  expires_at TIMESTAMP    NOT NULL,
  created_at TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  INDEX idx_revoked_expires (expires_at)
);

-- 4) refresh 标记（替换 SET_EX prefix:refresh:jti）
CREATE TABLE refresh_markers (
  jti        VARCHAR(128) PRIMARY KEY,
  expires_at TIMESTAMP    NOT NULL,
  created_at TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  INDEX idx_refresh_expires (expires_at)
);
```

**幂等写约束**：
- `refresh_markers` 写入使用 `INSERT IGNORE INTO refresh_markers (jti, expires_at) VALUES (?, ?)`：同一 jti 二次 mark 不报错，返回 affected_rows = 0，语义"已存在视为成功"。
- `revoked_jtis` 写入同样 `INSERT IGNORE`：重复 revoke 不出错。
- `auth_codes.code` 与 `oauth_clients.client_id` 为 PRIMARY KEY，重复 INSERT 会触发 1062，调用方应 catch `sqlx::Error::Database(e) if e.code() == "23000"` 并视为 idempotent。

### TTL 策略
- `db::init_pool` 启动后**必须**先 `SET time_zone = '+00:00'`：MySQL `NOW()` 默认按 session timezone；不强制 UTC 时 docker 容器与 host 时区不一致会导致 TTL 错判。连接池每次 acquire 都会重发该语句（包成中间件 / 用 sqlx `after_connect` hook）。
- 所有读查询统一 `WHERE expires_at > NOW()`（NOW() 在 UTC session 下等价 UTC 当前时间）
- 过期行暂不清除（数据量小，无须后台任务）
- `auth_codes.consume` 原子两步：
  1. `UPDATE auth_codes SET consumed_at = NOW() WHERE code = ? AND consumed_at IS NULL AND expires_at > NOW()`，affected_rows = 0 → 返回 `ServiceError::NotFound("code")`
  2. `SELECT user_id FROM auth_codes WHERE code = ?`（此时 consumed_at 已被本事务标记，user-service 远调用无需持锁）
  这样避免在事务内跨越 user-service HTTP 调用持锁，同时保证单次消费语义。

## 4. 默认客户端（Seed）

启动时若 `oauth_clients` 表为空，插入以下**四条**（保留 `api-gateway` 以兼容 frontend e2e 登录 spec `2026-06-11-frontend-e2e-login-design.md` 的 `client_id=api-gateway` + `VITE_CLIENT_SECRET=dev-gateway-secret` 流程）：

| client_id | permissions | grant_types |
|---|---|---|
| `api-gateway` | `token:issue`, `token:revoke` | `authorization_code`, `refresh_token` |
| `token-service` | `token:issue`, `token:revoke`, `token:refresh`, `user:read` | `authorization_code`, `refresh_token` |
| `user-service` | `user:read`, `user:write`, `role:read`, `permission:read` | `client_credentials` |
| `notification-service` | `notify:send`, `notify:read` | `client_credentials` |

- `client_secret_hash`：四条都使用 `DEFAULT_DEV_CLIENT_SECRET = "dev-gateway-secret"` + argon2 计算
- TTL 沿用现默认：`access_token_ttl_sec = 3600`，`refresh_token_ttl_sec = 2592000`（30 天）
- `status = 'active'`
- 启动时若表已存在但 hash 与 `DEFAULT_DEV_CLIENT_SECRET` 验证不通过，沿用旧 `maybe_upgrade_client_secret` 逻辑就地重算并更新

> ⚠️ **dev-only 警告**：`DEFAULT_DEV_CLIENT_SECRET` 与四客户端共用 secret 的方案**仅限开发与本地 docker-compose**。生产部署须：
> 1. 移除此 seed 逻辑（或在 production 编译 feature 下禁用）
> 2. 通过管理接口或手工 SQL 注入真实 hash
> 3. 各 client_id 配独立 secret，避免单点泄漏牵连
> 本期不实现 production-ready 密钥管理；仅记录此约束。

## 5. 文件结构与依赖

### 新增
```
services/token-service/
├── config/
│   └── config.yaml                    # YAML 替代 toml
├── migrations/
│   └── 20260611000001_init.sql        # 4 张表 DDL
├── src/
│   ├── db.rs                          # init_pool + DbPool（移植自 user-service，带 after_connect SET time_zone）
│   ├── secret.rs                      # hash_client_secret / maybe_upgrade / DEFAULT_DEV_CLIENT_SECRET（从 store/client.rs 迁出）
│   ├── models/
│   │   ├── mod.rs
│   │   ├── oauth_client.rs            # ClientRow : FromRow
│   │   ├── auth_code.rs               # AuthCodeRow
│   │   ├── revoked_jti.rs             # RevokedJtiRow
│   │   └── refresh_marker.rs          # RefreshMarkerRow
│   ├── service/
│   │   ├── mod.rs
│   │   ├── oauth_client_svc.rs        # find_by_id / list / seed_default_clients / insert
│   │   ├── auth_code_svc.rs           # save / consume（原子 UPDATE + SELECT user_id）
│   │   ├── revoke_svc.rs              # revoke（INSERT IGNORE）/ is_revoked
│   │   └── refresh_svc.rs             # mark（INSERT IGNORE）/ is_marked
│   └── store/
│       └── mysql.rs                   # 底层 sqlx 函数式调用（无 query! 宏）
└── tests/
    ├── oauth_client_test.rs
    ├── auth_code_test.rs
    ├── revoke_test.rs
    ├── refresh_marker_test.rs
    └── integration_test.rs            # 改：使用 MySQL 容器
```

### 修改
- `Cargo.toml`：
  - **删** `redis.workspace = true`
  - **删** `config.workspace = true`（TOML 加载不再用）
  - **删** `dotenvy.workspace = true`（YAML 配置不需要）
  - **加** `sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "tls-rustls-aws-lc-rs", "mysql"] }`（**不**含 `macros` feature，避免引入 `query!`/`query_as!` 编译期宏 —— 该宏需 `.sqlx/` 离线缓存或运行时 `DATABASE_URL`，与 user-service 保持函数式 API 一致）
  - **加** `serde_yaml = "0.9"`
  - **加** `anyhow = "1"`
  - 保留 `thiserror.workspace = true`、`argon2.workspace = true`、`rand.workspace = true`
- `src/lib.rs`：
  - `TokenServerState` 字段：`redis_conn: MultiplexedConnection` / `redis_prefix: String` → `db: DbPool`
  - 删除 `use redis::aio::MultiplexedConnection`
  - `redis` 相关 import 全部移除
- `src/main.rs`：
  - 加载 `config/config.yaml`（用 `serde_yaml::from_str`）
  - 删 `::config::Config::builder()` + `File::with_name("config/token-service")` + `Environment::with_prefix("TS")` 整段
  - 调用 `db::init_pool(&cfg.database)`（带 `after_connect` UTC TZ 设置）
  - 调用 `oauth_client_svc::seed_default_clients(&db)`（首次启动空表时 seed 4 客户端）
  - 构造 state 时传入 `db`
- `src/config.rs`：
  - 删 `use common::config::RedisConfig`（删除 `redis` import 后 `RedisConfig` 找不到，编译失败 —— 必须显式删除）
  - `TokenServerConfig` 字段：`redis: RedisConfig` → `database: DatabaseCfg`（与 user-service 同形：`db_type` / `url` / `max_connections`）
  - 删 `consul: ConsulConfig` 的 `default` impl 中所有引用；保留 consul 字段
- `src/store/mod.rs`：仅 `pub mod mysql;`
- `src/store/client.rs`：删除（`ClientStore` 内存缓存与 `load` / `reload` 逻辑不再需要，每次请求走 MySQL）
- `src/secret.rs`：**新增**（`hash_client_secret` / `maybe_upgrade_client_secret` / `DEFAULT_DEV_CLIENT_SECRET` 常量从 `store/client.rs` 迁出）
- `src/routes/access.rs`：
  - 删 `use crate::store::redis`
  - 删 `state.client_store` 引用，改 `oauth_client_svc::find_by_id(&state.db, &req.client_id)`
  - `redis::get_code` / `del_code` 改 `auth_code_svc::consume(&state.db, &req.code).await`（封装原子两步：UPDATE → SELECT user_id）
  - user-service 调用仍走 `state.http_client` + `state.user_service_url`
- `src/routes/refresh.rs`：
  - 删 `use crate::store::redis`
  - 删 `state.client_store` 引用
  - `redis::is_refresh_marked` / `add_refresh_family` 改 `refresh_svc::is_marked` / `mark`（mark 用 `INSERT IGNORE`）
- `src/routes/revoke.rs`：
  - 删 `use crate::store::redis`
  - `redis::add_to_blacklist` 改 `revoke_svc::revoke`（`INSERT IGNORE`）
- `src/routes/api_key.rs`：
  - **关键遗漏**：现 `state.client_store.add_client(new_client).await` 写内存；删 `ClientStore` 后必须改 `oauth_client_svc::insert(&state.db, new_client).await`（`INSERT INTO oauth_clients`）
  - DTO 校验逻辑不变
- `src/routes/resource.rs` / `jwks.rs` / `user_jwt.rs`：不变（不涉及存储）

### 删除
- `src/store/redis.rs`
- `src/store/client.rs`（`ClientStore` 缓存结构）
- `tests/redis_test.rs`
- 旧 `config/token-service.toml`

### 保留
- `issuer.rs`（JWT 签发）
- `consul.rs`（服务注册）
- RSA 密钥文件 `keys/private.pem` / `keys/public.pem`
- 所有对外 HTTP 路径

## 6. 错误处理

```rust
#[derive(thiserror::Error, Debug)]
pub enum ServiceError {
    #[error("not found: {0}")] NotFound(&'static str),       // auth_code / refresh_marker / jti 等
    #[error("conflict")] Conflict,
    #[error("forbidden: {0}")] Forbidden(&'static str),     // client disabled / grant_type 不支持
    #[error("invalid input: {0}")] InvalidInput(String),
    #[error("upstream unreachable: {0}")] UpstreamUnreachable(&'static str), // user-service HTTP 失败
    #[error("db error")] Db(#[from] sqlx::Error),
    #[error("internal")] Internal(#[from] anyhow::Error),
}
```

route 层 `ServiceError` → HTTP 映射（**与现行实现严格一致**）：

| 错误 | 状态码 | 触发路径（route 现状） | 响应消息 |
|---|---|---|---|
| `NotFound("code")` | 400 | `handle_access` code 不存在/已消费/过期 | `"invalid or expired code"` |
| `NotFound("client")` | 401 | `handle_access` / `handle_refresh` client_id 不存在 | `"invalid client"` |
| `Forbidden("client_disabled")` | 403 | `handle_access` `client.status != "active"` | `"client disabled"` |
| `Forbidden("grant_type")` | 400 | `handle_access` `grant_type != "authorization_code"` | `"unsupported grant_type"` |
| `Forbidden("refresh_used")` | 401 | `handle_refresh` refresh jti 已被 mark | `"refresh token already used"` |
| `InvalidInput(_)` | 400 | 字段缺失/格式错 | 透传 |
| `UpstreamUnreachable("user_service")` | 503 | `fetch_json` 失败（保持现状 503） | `"user_service_unreachable"` |
| `Db(_)` | 500 | sqlx 错误（read/write） | `"internal error"` + warn 日志 |
| `Internal(_)` | 500 | anyhow 未分类 | `"internal error"` + error 日志 |

**保留现状行为**：
- `handle_refresh` 中 `is_refresh_marked` 与 `add_refresh_family` 失败当前是 `.unwrap_or(false)` / `.ok()` 静默吞掉；切 MySQL 后保持同样宽容（不阻断 refresh 流程），但 service 层返回 `Result<bool>` 即可，调用方决定如何处理。
- `revoke` 当前 500 on redis error；切 MySQL 后保持 500，但底层错误归为 `Db(_)`。

## 7. 测试

| 文件 | 覆盖点 |
|---|---|
| `tests/oauth_client_test.rs` | seed 4 客户端、find_by_id 命中、未知 client 返回 None、insert 新 client |
| `tests/auth_code_test.rs` | save → consume（首次成功、第二次失败）、过期 code 不可消费、并发消费仅一次成功（`tokio::join!` 多任务） |
| `tests/revoke_test.rs` | revoke 后 is_revoked=true；TTL 内有效；过期不影响判定；重复 revoke 不报错 |
| `tests/refresh_marker_test.rs` | mark 后 is_marked=true；同一 jti 二次 mark 仍 true（`INSERT IGNORE` 幂等） |
| `tests/integration_test.rs` | 端到端：access / refresh / revoke 路径（MySQL 容器，顶部加 `// requires: cargo test --test-threads=1`，testcontainers 多任务并行会争 Docker 端口） |
| `tests/issuer_test.rs` | 保持原样（不依赖 DB） |

### 测试基础设施
- `testcontainers = "0.20"` + `testcontainers-modules` features = `["mysql"]`
- `sqlx::migrate!()` 跑 `migrations/20260611000001_init.sql`
- **必须**串行执行（`cargo test -- --test-threads=1` 或在 CI 配 `RUST_TEST_THREADS=1`）
- 测试期间不走 Consul
- `tests/integration_test.rs` 文件首行加注释： `// requires: cargo test --test-threads=1`

### dev-dependencies
```toml
[dev-dependencies]
testcontainers = "0.20"
testcontainers-modules = { version = "0.8", features = ["mysql"] }
reqwest.workspace = true
```

## 8. 配置示例

`services/token-service/config/config.yaml`：

```yaml
server:
  host: "0.0.0.0"
  port: 4000
database:
  db_type: mysql
  url: "mysql://token_service:token_pwd@mysql:3306/token_service"
  max_connections: 16
consul:
  addr: "http://consul:8500"
  service_name: "token-service"
  service_id_file: "/tmp/token-service-id"
jwt_private_key_path: "keys/private.pem"
jwt_public_key_path: "keys/public.pem"
issuer: "superpowers-token-service"
default_access_ttl_sec: 3600
default_refresh_ttl_sec: 2592000
default_resource_ttl_sec: 300
user_service_url: "http://user-service:8080"
```

## 9. 风险与回滚

- **风险 1**：MySQL 写性能低于 Redis。Mitigation：本期数据量小（auth_codes 短 TTL、refresh_markers 单 jti），QPS 远低于 MySQL 上限。
- **风险 2**：MySQL 不可用导致 token-service 启动失败。Mitigation：与现状 Redis 不可用等价；启动 panic 由 docker-compose 控制重启。
- **风险 3（高）**：MySQL session timezone 与 host 不一致导致 TTL 错判。Mitigation：`db::init_pool` 用 `after_connect` 强制 `SET time_zone='+00:00'`；同时 CI 与本地 docker-compose 必须显式 `TZ=UTC`。
- **风险 4（高）**：`DEFAULT_DEV_CLIENT_SECRET` 硬编码 + 四客户端共用 secret。Mitigation：仅 dev / docker-compose；生产部署需手工干预（见第 4 节警告）；本期在 `seed_default_clients` 函数顶部加 `#[cfg(debug_assertions)]` 或留 TODO 注记便于审计。
- **风险 5（中）**：`auth_codes` 过期行累积。30 天 TTL × 高 QPS 下 6 个月表可达百万行。Mitigation：本期接受；后续可加 cron `DELETE FROM auth_codes WHERE expires_at < NOW() - INTERVAL 1 DAY`。
- **风险 6（中）**：`user-service` 启动顺序：token-service 启动时不强依赖 user-service 在线，但首次 `/api/tokens/access` 调用需 user-service 在线。Mitigation：在 docker-compose 中将 token-service `depends_on: [user-service, mysql]`，user-service `depends_on: [mysql]`。
- **回滚**：保留 `store/redis.rs` 在 git 历史；`git revert` 可恢复。

## 10. 实施顺序（详细）

按依赖顺序：

1. **依赖**：`Cargo.toml` 加 sqlx/serde_yaml/anyhow，删 redis/config/dotenvy
2. **schema**：`migrations/20260611000001_init.sql` 写 4 表 DDL（带 `INDEX idx_*_expires`）
3. **db**：`src/db.rs` 写 `init_pool`（含 `after_connect` 强制 UTC）
4. **models**：`src/models/*.rs` 4 个 FromRow struct
5. **store**：`src/store/mysql.rs` 函数式 sqlx 调用
6. **service**：`src/service/*.rs` 4 文件 + 原子两步 consume
7. **secret**：`src/secret.rs` 迁出 client.rs 工具
8. **config**：`src/config.rs` 改 YAML + 删 RedisConfig import
9. **lib + main**：`src/lib.rs` / `src/main.rs` 改 state、加载 yaml、调用 init_pool + seed
10. **routes**：`access.rs` / `refresh.rs` / `revoke.rs` / `api_key.rs` 改调 service
11. **删除**：`store/redis.rs` / `store/client.rs` / `tests/redis_test.rs` / `config/token-service.toml`
12. **测试**：`tests/oauth_client_test.rs` / `auth_code_test.rs` / `revoke_test.rs` / `refresh_marker_test.rs`；改 `integration_test.rs` 走 MySQL 容器
13. **验证**（commit 前必须）：
    ```bash
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test -- --test-threads=1
    ```
14. **全量跑**：`.\scripts\test-orchestrator.ps1 loop`
