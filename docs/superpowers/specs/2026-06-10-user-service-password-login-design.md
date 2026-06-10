# User-Service 密码登录与安全策略 — 设计

日期：2026-06-10
状态：待审阅
服务：services/user-service
参考：`docs/superpowers/specs/2026-06-10-auth-login-security-design.md`（原 Go 设计，已合并到本 spec）

## 目标

将原 auth-service 登录安全策略迁移至 user-service，由 user-service 独自承担账号密码登录：

1. 为 `users` 表增加 `password_hash` / `last_login_at` / `last_login_ip`。
2. 新增 `user_login_log` 表，记录成功 / 失败登录。
3. 暴露 `POST /api/auth/login`：校验 Argon2id 哈希、Redis 防爆破计数与锁、签发 5 分钟 auth code。
4. `credential-login-service` stub 废除（其清理由独立 plan 负责）。

## 设计决策摘要

| 决策 | 选择 | 理由 |
|------|------|------|
| 哈希算法 | Argon2id | workspace 已有 argon2 0.5；AGENTS.md 已注明 user-service 用 Argon2 |
| 响应形式 | `{"code","expires_in":300}` | 与原设计一致；token-service 已用 `auth:code:` 交换 JWT |
| 端点位置 | user-service 直连 `/api/auth/login` | credential-login-service 废除 |
| 模块划分 | 独立 auth 模块 | 不污染 user CRUD；清晰分工 |
| 迁移方式 | SQL 文件 + `include_str!` | 与现有 `tests/integration_test.rs` 一致 |
| 计数器存储 | Redis | 跨实例事实来源；与原设计一致 |
| 计数 key 维度 | `auth:login:fail:<username>` | 账号存在与否都计数；防枚举 |
| 锁 key 维度 | `auth:login:lock:<username>` | 与计数 key 分离，独立 TTL |
| 锁定响应 | HTTP 423 + `retry_after` | 与原设计一致；语义清晰 |
| 表名 | `users` / `user_login_log` | 沿用 Rust 代码现查名；不沿用旧 Go `usr_*` / `member_user` 前缀 |

## 配置

`config/config.yaml` 新增段：

```yaml
login_security:
  max_failures: 6
  lockout_duration: 5m
  counter_window: 15m
  code_ttl: 5m
  argon2:
    memory_kib: 19456
    iterations: 2
    parallelism: 1
```

`config.rs` 新增：

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Argon2Cfg {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for Argon2Cfg {
    fn default() -> Self {
        Self { memory_kib: 19456, iterations: 2, parallelism: 1 }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LoginSecurityCfg {
    pub max_failures: u32,
    pub lockout_duration: Duration,
    pub counter_window: Duration,
    pub code_ttl: Duration,
    pub argon2: Argon2Cfg,
}

impl Default for LoginSecurityCfg {
    fn default() -> Self {
        Self {
            max_failures: 6,
            lockout_duration: Duration::from_secs(300),
            counter_window: Duration::from_secs(900),
            code_ttl: Duration::from_secs(300),
            argon2: Argon2Cfg::default(),
        }
    }
}
```

`Config` 增 `pub login_security: LoginSecurityCfg`，serde `#[serde(default)]` 保证旧配置无该段时使用默认值。

## 数据模型

迁移文件 `migrations/2026_06_10_add_password_and_login_log.sql`：

```sql
USE oa3;

ALTER TABLE users
  ADD COLUMN password_hash VARCHAR(255) NULL AFTER user_code,
  ADD COLUMN last_login_at DATETIME NULL,
  ADD COLUMN last_login_ip VARCHAR(45) NULL;

CREATE TABLE IF NOT EXISTS user_login_log (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
  user_id BIGINT UNSIGNED NULL,
  username VARCHAR(64) NOT NULL,
  ip VARCHAR(45) NOT NULL,
  user_agent VARCHAR(255) NOT NULL,
  success TINYINT(1) NOT NULL,
  fail_reason VARCHAR(32) NULL,
  login_at DATETIME(3) NOT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
  INDEX idx_login_log_username (username),
  INDEX idx_login_log_user_id (user_id),
  INDEX idx_login_log_success (success),
  INDEX idx_login_log_login_at (login_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
```

字段语义：
- `users.password_hash`：完整 PHC 字符串 `$argon2id$v=19$m=...,t=...,p=...$salt$hash`，长度 255。
- `users.last_login_at` / `last_login_ip`：成功后更新。
- `user_login_log.user_id`：失败且账号不存在时 NULL。
- `user_login_log.fail_reason`：`user_not_found` / `bad_password` / `locked`，成功时 NULL。

## 模块布局

不动 user-service 现有 user CRUD。新增：

```
services/user-service/src/
├── models/
│   ├── login_log.rs       # LoginLogRow 结构
│   └── mod.rs             # +pub mod login_log
├── service/
│   ├── auth_svc.rs        # login 业务：lock → user fetch → argon2 → counter → log → code
│   └── mod.rs             # +pub mod auth_svc
└── routes/
    ├── auth.rs            # POST /api/auth/login 处理器
    └── mod.rs             # +pub mod auth
```

`auth_svc::login()` 不暴露为内部 HTTP；走 axum route 直接挂载。`src/auth.rs`（已存在的内部 service-token 中间件）保留。

## 登录流程

```
POST /api/auth/login  body: {username, password}
        │
        ▼
1. 解析 & 必填校验
   - 缺字段 → 400 (AppError::Validation)
        │
        ▼
2. Redis GET auth:login:lock:<username>
   - 存在 → 写 LoginLog(success=0, fail_reason=locked)
         → 423 {"error":"account locked","retry_after":<TTL 剩余秒>}
        │
        ▼
3. SELECT id, password_hash FROM users WHERE username=? AND status=1
   - 不存在 → argon2 verify dummy_hash 走失败分支（时长对齐）
        │
        ▼
4. argon2 verify password_hash(password)
   - false → 失败分支
   - true → 成功分支
        │
        ▼
5. 失败分支：
   a) INCR auth:login:fail:<u>；若返回 ==1 → EXPIRE counter_window
   b) 若 n >= max_failures → SET auth:login:lock:<u> 1 EX lockout_duration
   c) INSERT user_login_log (success=0, fail_reason, user_id=?, username, ip, ua, login_at=now)
   d) 返回 401 {"error":"invalid credentials"}
        │
        ▼
6. 成功分支：
   a) DEL auth:login:fail:<u> + DEL auth:login:lock:<u>（防御性）
   b) UPDATE users SET last_login_at=now, last_login_ip=? WHERE id=?
   c) INSERT user_login_log (success=1, user_id, username, ip, ua, login_at=now)
   d) 生成 code = nanoid(32)
      SET auth:code:<code> = user_id EX code_ttl
   e) 返回 200 {"code":"<nano>","expires_in":300}
```

关键约束：
- `<username>` 用请求体原始字符串，不归一化，避免绕过。
- 锁判定在用户查找之前：已锁账号直接拒，减少 DB 查询。
- 账号不存在也走 argon2 verify（dummy hash），时长对齐防时序枚举。
- 日志写入失败用 `tracing::warn!` 记录，不阻塞主流程。
- `auth_svc::fetch_user_for_login` 独立函数，不复用 `fetch_user_with_roles`，避免 NotFound 副作用。

## Redis key 设计

| Key | 类型 | TTL | 用途 |
|-----|------|-----|------|
| `auth:login:fail:<u>` | int | `counter_window` | 失败计数 |
| `auth:login:lock:<u>` | string "1" | `lockout_duration` | 锁定标记 |
| `auth:code:<code>` | int (user_id) | `code_ttl` | login → token-service 交换 |

## 错误码

| 场景 | HTTP | body |
|------|------|------|
| 缺字段 | 400 | `{"code":"VALIDATION","message":"..."}` |
| 账号被锁 | 423 | `{"error":"account locked","retry_after":<秒>}` |
| 账号或密码错 | 401 | `{"error":"invalid credentials"}` |
| 成功 | 200 | `{"code":"<nano>","expires_in":300}` |

`AppError` 不新增变体；423 / 401 由 `routes::auth::login` 直接返回 `axum::Json`+`StatusCode`。错误文案模糊，不暴露账号是否存在。

## IP 与代理

通过 `axum::extract::ConnectInfo` 或 `tower_http::ServiceRequestExt::connect_info` 获取客户端 IP。`main.rs` 不做额外代理信任配置；交由 api-gateway 层处理。

## 文件改动清单

- `services/user-service/Cargo.toml` — `+argon2 = { workspace = true }`，`+nanoid`
- `migrations/2026_06_10_add_password_and_login_log.sql` — 新建
- `services/user-service/config/config.yaml` — `+login_security` 段
- `services/user-service/docker/config/user-service/config.yaml` — 同步
- `services/user-service/src/config.rs` — `+LoginSecurityCfg` / `+Argon2Cfg` / `+Default` 实现
- `services/user-service/src/models/login_log.rs` — 新建
- `services/user-service/src/models/mod.rs` — `+pub mod login_log`
- `services/user-service/src/service/auth_svc.rs` — 新建（login / record_attempt / is_locked / incr_failure / clear_failure / write_log / issue_code / fetch_user_for_login）
- `services/user-service/src/service/mod.rs` — `+pub mod auth_svc`
- `services/user-service/src/routes/auth.rs` — 新建（login handler + LoginReq DTO）
- `services/user-service/src/routes/mod.rs` — `+pub mod auth`
- `services/user-service/src/dto.rs` — `+LoginReq`
- `services/user-service/src/main.rs` — 挂 `Router::new().route("/api/auth/login", post(routes::auth::login))`
- `services/user-service/tests/auth_test.rs` — 新建（集成测试）
- `docker/config/api-gateway/routes/*.router.toml` — `+/api/auth/login` → user-service

## 测试策略

`tests/integration_test.rs` 沿用 testcontainers-mysql 模式；新增 `tests/auth_test.rs` 用 testcontainers redis + mysql 跑：

1. 失败 6 次 → 第 7 次 423 + retry_after
2. 成功 1 次 → 计数器与锁 key 清空
3. 账号不存在也计数（fail_reason=user_not_found）
4. 写日志：成功 / 失败 user_login_log 各 1 行
5. last_login_at / last_login_ip 成功后更新
6. argon2 verify 单次 < 500ms

每个集成测试用独立 username + Redis db 1 隔离。

## 不做的事

- 不做 IP 维度限流（防同 IP 多账号爆破不在本设计）
- 不做验证码 / 二次认证
- 不做滑动窗口或指数退避
- 不引入新中间件或库（沿用 axum + redis + sqlx + argon2）
- 不动 token-service 交换逻辑
- 不动 user-service 现有 CRUD 路由
- 不删 `services/credential-login-service/`（由独立清理 plan 负责）

## 风险与注意

- **跨实例**：Redis 是单点事实来源，无需本地缓存。
- **Redis 重启**：未到 TTL 计数丢失，最坏用户多试一次。可接受。
- **argon2 选参**：memory=19MiB / t=2 / p=1 单次约 30-80ms；若压测偏高可调低 memory 至 12MiB。
- **日志写失败**：用 `tracing::warn!` 记录，不阻塞登录返回。
- **时区**：MySQL DSN `loc=Local`，DATETIME 写入与查询一致。
- **路由校验**：新增 `routes/*.router.toml` 后必须先 `cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes` 再 commit。
- **沙箱限制**：所有 `cargo build / cargo test` 在真实 PowerShell（vcvarsall x64）执行；测试由 `coder-run-tests` 触发。
