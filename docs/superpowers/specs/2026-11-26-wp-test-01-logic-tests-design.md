# WP-TEST-01：无基础设施逻辑测试体系设计

- 日期：2026-11-26
- 基线：main @ 28b3a0c8
- 分支：feat/wp-test-01-logic-tests

## 1. 目标与范围

建立 `pnpm run test:logic`：在不启动、不连接 MySQL / PostgreSQL / Redis /
Consul / RabbitMQ / Docker Compose 的前提下，运行全部 PURE / MEMORY /
SQLITE 类测试。原基础设施集成测试（MYSQL/REDIS/RABBITMQ/PG）原样保留。

### 分类语义（本任务约定）

| 类别 | 定义 | 判据 |
|---|---|---|
| PURE | 进程内纯逻辑，无任何 IO | `#[cfg(test)]` 单测；oneshot 路由测试 |
| MEMORY | InMemory / Fake / wiremock / 本机 loopback HTTP | 不依赖任何外部服务端 |
| SQLITE | 嵌入式 SQLite（`sqlite::memory:` / tempfile） | 无服务端，仅文件系统 |
| MYSQL / REDIS / RABBITMQ | 需对应服务端（共享实例或 testcontainers） | spawn_mysql / REDIS_URL / testcontainers |
| POSTGRES | 需真实 PG（`TEST_PG_URL` gate） | 未设 env 打印 skip |
| E2E | 跨服务真实环境 | scripts/run-e2e-local.js 等 |

## 2. 审计结论（分类清单）

### 2.1 已验证的 PURE 基线

`cargo test --workspace --lib -j 4`（main @ 28b3a0c8）本机无任何基础设施
**228 passed / 0 failed**。全部 18 个 libs crate + 8 个服务的
`#[cfg(test)]` 单测中，除 `common-http/src/resilient.rs`（loopback axum，
自包含）外均为纯逻辑。

### 2.2 PURE / MEMORY 集成测试白名单（`tests/*.rs`）

**libs（按 crate → 文件）**：

- authz: `verify_aud_test`（oneshot）
- common: `password_test`
- common-cache: `cache_test`, `tiered_cache_test`（手写 `MockRemoteCache impl Cache`）
- common-config: `config_test`, `hot_config_test`, `provider_test`（wiremock）
- common-consul: `registration`（wiremock mock Consul + env 契约，自包含）
- common-crypto: `crypto_test`
- common-db: `dbpool_test`, `dialect_test`, `sql_render_test`, `scope_test`,
  `query_macro_test`（sqlite::memory ×9 + PG env gate ×1）, `sqlite_fixture_test`
- common-error: `error_test`
- common-event: `outbox_sql_test`, `enqueue_trace_id_test`, `error_classes`,
  `events_codegen`, `event_trait`（5 个 PURE；其余 7 文件为 tc 容器类，排除）
- common-health: `health_test`
- common-http: `http_test`, `resilient_test`, `resilient_streaming_test`（loopback）
- common-jwt: `cache_test`, `verify_test`, `service_jwt_test`（wiremock）
- common-log: `log_test`, `audit_test`
- common-ratelimit: `unit_test`
- common-telemetry: `current_trace_id_test`
- common-telemetry-axum: `access_log_trace_id`
- common-validator: `validator_test`
- jwt-ctx: `extractor_test`, `principal_test`, `principal_tenant_test`,
  `scope_test`, `from_token_test`

**services**：

- api-gateway: `integration_test`, `template_test`, `middleware_test`,
  `jwt_test`, `routes_loader_test`, `tracing_mw_test`, `resilience_test`,
  `router_streaming_test`, `rate_limit_test`（3 PURE + 2 已 #[ignore]）,
  `blacklist_test`（本次加 `REDIS_URL` gate，见 §4.1）
- audit-service: `models_test`
- file-service: `error_codes_test`, `models_test`, `local_storage_test`
- iam-service: `integration_test`（connect_lazy 懒连接从不拨号）,
  `change_password_test`, `swagger_test`
- notification-service: `recipient_resolver_test`, `rate_limit_config_test`,
  `iam_client_test`（wiremock）
- prue_backend: `menu_config_model_test`, `menu_config_service_test`
  （`sqlite::memory:` 服务级样板）
- token-service: `issuer_test`, `swagger_test`

### 2.3 基础设施类（排除在 test:logic 之外，原样保留）

| 类别 | 位置 |
|---|---|
| MYSQL（spawn_mysql/spawn_db，共享库或 testcontainers） | token ×17、iam ×9、notification ×4、file tenant_isolation、api-gateway 无、common-event outbox_repo/enqueue_trace_id_to_db |
| RABBITMQ（testcontainers） | common-event ×3（consumer_redelivery / ephemeral_subscribe / rabbit_integration）、audit login_risk_test |
| REDIS（testcontainers / REDIS_URL） | common-event perm_epoch、iam permission_changed / user_roles_assign、notification welcome_consumer |
| POSTGRES（TEST_PG_URL gate） | 各服务 pg_roundtrip_test、token pg_grant_unique_test、common-db pg_test / pg_migration_test、common-event outbox_pg_test |
| api-gateway blacklist_test | 原 5 用例直连 `REDIS_URL`（127.0.0.1:6379）无保护 → §4.1 修复后归 REDIS（gate 类） |

**本机固有失败澄清**：`common-event consumer_redelivery`（6 用例）在本机
失败系缺 RabbitMQ 端点（干净 main 同样失败），与本任务无关。

## 3. `pnpm run test:logic` 设计

- 入口：`scripts/test-logic.js`（Node，无第三方依赖）。
- 阶段 A：`cargo test --workspace --lib -j 4`（全部 src 单测 = PURE 主体）。
- 阶段 B：按 §2.2 白名单逐包 `cargo test -p <pkg> --test <file...>`。
- 环境纪律：
  - 脚本启动时检测 `TEST_MYSQL_HOST/TEST_MYSQL_PORT/TEST_PG_URL/REDIS_URL/DATABASE_URL`
    并显式 **unset**（防开发机 env 泄漏导致"纯"测试意外连库——例如
    common-db `query_macro_test` 的 PG 模块会因 TEST_PG_URL 而真连）；
  - `SWAGGER_UI_DOWNLOAD_URL` 未设置时自动注入 vendor
    `infrastructure/vendor/swagger-ui-v5.17.14.zip` 的 `file://` URL
    （utoipa-swagger-ui build.rs 需要离线 zip）；
  - 不使用 `--no-default-features`（各包 default feature 即测试所需最小集）。
- 清单治理：`--check` 模式扫描 `libs/*/tests/*.rs` 与 `services/*/tests/*.rs`，
  凡不在 PURE 白名单也不在 infra 排除清单中的文件 → 警告输出（不失败），
  防止新测试文件漂移无人分类。
- 失败语义：任何用例失败 → 非 0 退出；不用 skip/ignore 规避既有断言。

## 4. 测试资产修订（仅必要改动）

### 4.1 api-gateway blacklist_test 加 `REDIS_URL` gate

现状：5 用例直连 `REDIS_URL`（缺省 127.0.0.1:6379），无 Redis 环境**必挂**
（ai-context.md pitfall 明示这是历史遗留失败）。修复：采用仓库既有的
`TEST_PG_URL` skip-if-unset 惯例——未设 `REDIS_URL` 时打印 `[skip]` 通过；
设了则行为与断言**逐字节不变**。这不是规避失败，而是把 Redis 集成测试
正确归入需要 Redis 的类别（与 pg_roundtrip 同型）。

### 4.2 blacklist key 契约纯函数（最小 MEMORY 补充）

`src/blacklist.rs` 抽出 `pub fn blacklist_key(prefix, jti) -> String`
（`is_blacklisted` 改为调用它，3 行生产改动、零语义变化），新增
`tests/blacklist_key_test.rs`：锁死 `{prefix}blacklist:{jti}` 拼装契约
（前缀大小写、特殊字符 jti、空段），防止 middleware 与 revoke 写入两侧
key 漂移。

### 4.3 Fake / InMemory 现状与边界

既有可替换点（已覆盖测试）：`Cache` trait + `MockRemoteCache`（common-cache）、
`ResilienceObserver` + `RecordingObserver`（common-http）、
`PrincipalStateProvider`（jwt-ctx/authz）、`MigrationState`（common-db）、
`OutboxRepo/Event` trait（common-event）、`ObjectStore`（file-service）、
`LocalKms`（common-crypto）、wiremock 桩（config/consul/jwt/iam-token）。

**不做** Repository/服务层大规模 trait 化（任务书明令禁止大规模重构）；
token-service store 层（占位）与 iam/notification service 层的 InMemory
化列为后续任务（见 §7）。

## 5. CI Job

- GitHub Actions：新增 `.github/workflows/logic-tests.yml`——
  `test-logic` job，**无任何 `services:` 段**（不配 MySQL/PG/Redis 容器），
  steps = checkout + rust toolchain + cache（沿 PR #20 模式）+
  `node scripts/test-logic.js`。PR 与 main push 触发。
- GitLab CI 不新增：现有 `rust_ci` 已跑 `cargo test --workspace --lib`
  （PURE 主体，无服务容器），避免 runner 资源重复消耗；报告中说明。

## 6. 验收口径

- `pnpm run test:logic` 在无 Docker、无任何 DB/Redis/Consul/RabbitMQ 的
  环境全绿；记录用例数与耗时。
- 原有集成测试零改动（除 blacklist_test 的 env gate 与 §4.2 的 3 行）；
  `cargo test --workspace --lib`、dialect-matrix、rust_ci 等现有门禁不变。
- `--check` 清单治理输出"无未分类文件"。

## 7. 后续任务建议（不在本任务范围）

1. token-service store 层抽 trait + InMemory 实现（17 个 MYSQL 文件中
   grant/bootstrap 管理面可先迁）。
2. iam/notification service 层 `Arc<dyn OutboxRepo>` 化（libs trait 已备）。
3. common-ratelimit Redis 版 RateLimiter 的判定逻辑抽纯函数 + InMemory 替身
   （现为全仓最明显零覆盖点）。
4. api-gateway `AppState.redis_conn` 具体类型 → `BlacklistStore` trait。
5. SQLITE 类推广：iam/token 迁移无 sqlite 目录，若需要服务级 SQLITE 测试
   需先补双栈迁移（prue 是唯一双栈服务）。
