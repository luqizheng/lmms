# WP-TEST-01-R1：Token Blacklist 存储解耦与 InMemory 回归（DEC-02 执行报告）

- 日期：2026-11-27
- 依据：DEC-02 Owner 决策（PR #21 评审后追加，12 条验收要求）
- 分支：feat/wp-test-01-logic-tests（延续 PR #21）
- 前置：WP-TEST-01 主设计 `2026-11-26-wp-test-01-logic-tests-design.md`

## 1. 设计结论：复用既有存储抽象，未新增 trait

DEC-02 第 3 条"优先复用现有存储抽象"。调研结论：`digua_common_cache`
已具备完整三件套——

| 组件 | 位置 | 角色 |
|---|---|---|
| `Cache` trait（`#[async_trait]`，dyn 安全，`get/set(ttl)/del/incr/mget`） | `libs/common-cache/src/lib.rs` | 既有存储抽象 |
| `MemoryCache` | `libs/common-cache/src/memory.rs` | InMemory 替身（常规 CI） |
| `RedisCache` / `SharedRedisCache` | `libs/common-cache/src/redis.rs` | 真实 Redis 适配（保留生产实现） |

因此**不新设计 trait**，只在 `Arc<dyn Cache>` 之上新增薄业务服务：

- `digua_common_cache::blacklist::TokenBlacklistService`
  （`libs/common-cache/src/blacklist.rs`）：`revoke(jti, ttl)` /
  `is_revoked(jti)`。
- `digua_common_cache::blacklist::blacklist_key(prefix, jti)`：key 契约
  `{prefix}blacklist:{jti}` 的**唯一真相源**——gateway 读取侧与 iam 写入侧
  从此共用同一实现（修复 WP-TEST-01 评审 P2#1"契约只锁单侧"）。
- `SharedRedisCache<C: ConnectionLike>`：复用调用方既有共享连接
  （gateway 的 `MultiplexedConnection` / iam 的 `ConnectionManager`），
  避免引入每请求新建连接的 `RedisCache` 造成生产性能回退。

### 过期语义（DEC-02 第 4/11 条的关键边界）

过期判断在**业务层**实现：`revoke` 把过期时刻（epoch ms）编码进 value，
`is_revoked` 读取后与注入时钟比较（`now == until` 即失效）。存储侧同时
收到 TTL（真实 Redis 用它做条目回收），但业务正确性不依赖存储 TTL。

⇒ Mock 路径验证的是**业务层过期判断**；Redis TTL 回收、值序列化、
网络行为**只有真实 Redis 集成测试能覆盖**，Mock 测试与注释均未声称覆盖。

## 2. 接线改动

| 侧 | 文件 | 改动 |
|---|---|---|
| 读 | `services/api-gateway/src/middleware/mod.rs` | `is_blacklisted(conn,...)` → `state.blacklist.is_revoked(jti)`；错误策略保持既有（见 §4） |
| 读 | `services/api-gateway/src/lib.rs` | `AppState` 新增 `blacklist: TokenBlacklistService` 字段 |
| 读 | `services/api-gateway/src/main.rs` | `SharedRedisCache::new(redis_conn)`（复用启动期连接）构造 service |
| 读 | `services/api-gateway/src/blacklist.rs` | 瘦身为 `connect()` + 契约 re-export（`blacklist_key_test.rs` 导入路径不变） |
| 写 | `services/iam-service/src/service/auth_svc.rs` `logout()` | 手写 `format!` + `set_ex("1")` → `TokenBlacklistService::revoke(jti, ttl)`；错误策略保持既有 best-effort + warn（见 §4） |
| 依赖 | 两服务 `Cargo.toml` | `digua_common_cache` 加 `features = ["redis"]` |

**既有行为变化备案**：黑名单 value 从字面 `"1"` 变为过期时刻字符串；gateway
读取从 `EXISTS` 变为 `GET`+解码。开发阶段无生产数据（AGENTS.md 约定），
不做旧值兼容；若滚动重启窗口内存在旧值（纯数字，如 `"1"`），会被当作
"过期时刻=1ms" 视为未撤销——影响仅限旧 token 的剩余 TTL（分钟级）。

## 3. 测试资产（DEC-02 第 4/5/8/9 条）

### 3.1 业务逻辑回归（新增，MEMORY，常规 CI 执行）


`libs/common-cache/tests/blacklist_service_test.rs`（11 用例，全部真实
执行于 test:logic 与 rust_ci `--workspace --lib` 之外的 tests 目标）：

| DEC-02 第 5 条要求 | 用例 |
|---|---|
| revoke → is_revoked | `revokes_then_reports_revoked` |
| 过期 | `expires_when_clock_passes_until_boundary`（注入时钟，含 `now==until` 边界） |
| 未知 JTI | `unknown_jti_is_not_revoked` |
| 不同 JTI 隔离 | `different_jtis_are_isolated` |
| （补充）前缀隔离 | `different_prefixes_do_not_collide` |
| （补充）重复 revoke | `re_revoke_extends_until` |
| （补充）key 契约 | `writes_under_the_shared_key_contract` |
| （补充）生产构造冒烟 | `system_clock_constructor_smoke` |

同一业务服务（`TokenBlacklistService`）同时承载读写两侧语义，无平行
测试替身。

### 3.2 错误策略（DEC-02 第 6 条）

`FailingCache`（读/写恒错）注入：

- `read_failure_propagates_instead_of_silently_denying_or_allowing`：
  存储读故障 → service 返回 `Err`（不伪装成"未撤销"）。
- `write_failure_propagates_instead_of_reporting_revocation_success`：
  存储写故障 → service 返回 `Err`（不谎报撤销成功）。
- `corrupt_value_propagates_error_not_false`：非数字损坏值 → `Err`
  （不默认放行）。

**本条与既有策略的关系（重要）**：api-gateway middleware 在
blacklist 检查出错时的既有安全策略是 **fail-open + warn**
（2026-07-14 `1910c405` 提交信息明示："blacklist 检查失败时 fail-open
并 warn 日志，避免 redis 故障阻断全部请求"）。本 R1 按"遵守既有安全
策略"条款**原样保留** fail-open，未改动；按第 6 条的精神，测试**不固化**
放行行为——放行决策留在中间件层、仅以日志与注释指认，不写成断言。
⇒ 若 Owner 认为 Redis 故障期间应改为 fail-closed（拒绝请求），需另行
DEC；改动点是 middleware `mod.rs` 中 `Err(e) => false` 的分支。

iam 写侧既有策略同样是 best-effort：logout 的持久撤销（refresh token
`revoked=TRUE`）先完成，blacklist 写失败仅 warn、logout 仍成功。语义上
"撤销成功"指 DB 撤销；blacklist 未生效的风险 = 当前 access token 存活
至自然过期（分钟级 TTL）。已如实备案，未谎报。

### 3.3 真实 Redis 集成测试（保留，REDIS 类）

- `services/api-gateway/tests/blacklist_test.rs`：5 用例断言逐字节未动，
  未 Mock 化（第 8 条 ✓）。
- 已从 test:logic 的 PURE/MEMORY 白名单移除，归入 `INFRA_TEST_FILES`
  （REDIS 类）（第 9 条 ✓）；`node scripts/test-logic.js --check` 绿。
- 头注释写明本地执行方法（第 10 条）：

  ```bash
  pnpm run dev:infra        # 或任一可用 Redis
  REDIS_URL=redis://127.0.0.1:36379 cargo test -p api_gateway --test blacklist_test
  ```

## 4. 本机执行记录（DEC-02 第 10/12 条）

- 常规 CI 等价验证：`node scripts/test-logic.js`（阶段 A 全 workspace
  `--lib` + 阶段 B 白名单，含新 `blacklist_service_test` 11 用例、不含
  任何基础设施）→ 结果见提交信息/PR CI（test-logic job）。
- **真实 Redis 实跑**（2026-11-27 本机，临时容器
  `wp-test-01-r1-redis`（redis:7-alpine，映射 36379），跑完即删）：
  已执行，**两组全部通过**——

  1. `REDIS_URL=redis://127.0.0.1:36379 cargo test -p api_gateway --test blacklist_test`
     → `5 passed; 0 failed`（2.06s，含 TTL 过期用例）。
  2. 生产适配器 SharedRedisCache 真实往返（新增
     `libs/common-cache/tests/shared_redis_cache_test.rs`，REDIS 类）：
     `REDIS_URL=... cargo test -p digua_common_cache --features redis --test shared_redis_cache_test`
     → `2 passed; 0 failed`（set/get/del 往返 + `TTL` 实测 SET_EX
     侧过期生效）。该文件已列入 INFRA_TEST_FILES（REDIS 类），
     常规 CI 不执行。

  临时容器验证后已 `docker stop` 删除，无残留。

## 5. Mock 覆盖范围与真实 Redis 未覆盖风险（DEC-02 第 12 条）

**Mock（MemoryCache + 注入时钟）覆盖**：key 拼装契约、撤销写入、撤销
判定、业务层过期（含边界）、未知 JTI、前缀/JTI 隔离、重复撤销、损坏值
报错、存储读/写故障如实上抛、生产构造路径冒烟。

**真实 Redis 仅覆盖、Mock 不覆盖（残余风险）**：

1. Redis TTL 回收（`SET ... EX` 真实过期、`TTL` 语义、到期删除）。
2. 值序列化落盘格式（`GET` 返回字节 ↔ `Vec<u8>` 往返）。
3. 网络行为：连接建立/复用（`MultiplexedConnection` 共享连接并发）、
   断连重连、超时。
4. 多实例部署下 key 的一致可见性（Mock 是进程内）。
5. 滚动部署窗口内旧格式值（`"1"`）的放行行为（§2 备案，仅真实环境
   可复现）。

**策略风险（非测试缺口，报 Owner 备案）**：读侧 fail-open 意味着 Redis
故障期间已撤销 token 可继续使用至自然过期——这是 2026-07-14 的既有可用
性决策，本 R1 未改变；如需收紧请另行 DEC。

## 6. DEC-02 十二条对照

| 条 | 要求 | 状态 |
|---|---|---|
| 1 | 不新增 Redis service/专用 job | ✓ 未改任何 workflow |
| 2 | 不改逻辑测试 CI 隔离 | ✓ logic-tests.yml 未动 |
| 3 | 复用既有抽象/最小 trait | ✓ 复用 `Cache`，零新 trait |
| 4 | 提供 InMemory 实现 | ✓ `MemoryCache`（既有）+ service 注入时钟 |
| 5 | 同一业务服务验证五行为 | ✓ §3.1 |
| 6 | 错误遵守既有策略、不默认放行/谎报 | ✓ service 层如实上抛；fail-open 保留并备案（§3.2/§5） |
| 7 | 保留真实 Redis Adapter | ✓ `RedisCache`/`SharedRedisCache` 生产路径 |
| 8 | 保留 blacklist_test 真实测试 | ✓ 断言零改动 |
| 9 | 移出 PURE/MEMORY 白名单归 REDIS | ✓ `INFRA_TEST_FILES` |
| 10 | 本地执行方法 + 实际结果 | ✓ 文件头注释 + §4 实跑记录 |
| 11 | 不冒充 Redis 行为 | ✓ 文件/注释/本报告三处显式声明 |
| 12 | Mock 覆盖范围与风险清单 | ✓ §5 |
