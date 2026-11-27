# WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-02 交付报告

日期：2026-11-27
仓库：`https://github.com/luqizheng/lmms.git`
目标 PR：#19（`feat/wp-lmms-service-jwt-audience-admin-01`）

---

## A. 基线

| 项 | 值 |
|---|---|
| 原始 PR HEAD（任务书基线） | `7caefd3ef5dbe92b9ca143724b786a3576e4733f` |
| 实际开发基线 | `7caefd3ef5dbe92b9ca143724b786a3576e4733f`（开工前经 `gh pr view 19` 实测确认，未发生变化） |
| 分支名称 | `feat/wp-lmms-service-jwt-audience-admin-02`（自 PR #19 分支新建的独立 feature branch） |
| 修复 commit | `e26cf465f9f42139a7d8b5be752e4fdabc73f68d` |
| 最终 HEAD（含本报告） | 本报告提交后的分支 tip（见 commit 2 的 log；修复本体为上一行 SHA） |

两个 P1 均在 commit `e26cf465` 完成，本报告（交付文档）单独成 commit 以便记录最终 SHA。

## B. P1-01 修复说明：Service JWT 管理入口权限边界

### 原始权限风险

`/admin/service-jwt-grants*` 五条管理路由仅由 `verify_aud_principal_middleware("token-service")`
保护：**任何**拥有 `aud=token-service` service-jwt 的普通业务服务（如持有
`notification-service → token-service` 类 grant 的主体）都能创建、启停、删除
任意 owner 的 grant——等价于获得全局服务间授权管理能力。

### 新的权限模型

- **普通服务权限** ≠ **grant 管理权限**。`aud=token-service` 只证明"有权访问
  token-service"；管理 grant 需要额外的部署级管理员身份。
- **管理权限来源（可信、可验证）**：`TOKEN_SERVICE_JWT_GRANT_ADMINS`
  （env，逗号分隔）/ `service_jwt_grant_admins`（yaml）—— canonical bootstrap
  owner 清单，与 `SERVICE_BOOTSTRAP_TOKENS` 同级部署信任，随部署配置审计。
  **空清单 = fail-closed：所有 service-jwt 全部 403**（不因缺配置放权）。
- **调用者身份来源（客户端不可伪造）**：service-jwt 的 `sub` claim——由
  token-service 签发时从 DB `token_bootstrap_tokens.owner` 写入、RS256 验签 +
  aud 强制（既有中间件）保护；经 `Principal::user_id` 提取后做
  `canonical_owner` 归一（剥宽限期 `#old`）与清单比对。
- **请求参数不参与判定**：owner / audience / status 等请求体与路径参数只是
  操作目标，修改它们无法获得管理能力。
- **Web UI 入口不受影响**：`/v1/admin/service-jwt-grants*` 的 user-jwt +
  权限码（`TOKEN_SERVICE_JWT_GRANT_READ/CREATE/UPDATE/DELETE`）机制原样保留，
  未削弱。
- 未新增权限码（service-jwt 无 permissions claim，任务书禁止改签发协议）；
  未扩大任何普通服务的默认权限（默认 = 无管理权限）。

### 各管理路由的权限检查位置

统一实现在 `services/token-service/src/routes/service_jwt_grant_admin.rs`
的 `require_grant_admin()`，在每个 handler 的**第一行**（任何业务读写与
审计动作之前）调用：

| 路由 | handler | 检查位置 |
|---|---|---|
| GET `/admin/service-jwt-grants` | `handle_list` | `require_grant_admin` → 403，仅 log |
| GET `/admin/service-jwt-grants/{owner}/{audience}` | `handle_get` | 同上 |
| POST `/admin/service-jwt-grants` | `handle_create` | `require_grant_admin` 失败 → best-effort denied 审计 + 403 |
| PUT `/admin/service-jwt-grants/{owner}/{audience}/status` | `handle_set_status` | 同上 |
| DELETE `/admin/service-jwt-grants/{owner}/{audience}` | `handle_delete` | 同上 |

5 个 handler 的 utoipa 注解同步新增 403 响应，`openapi.yaml` 已重生成。

### 普通 Service JWT 拒绝证据（测试）

全部在 `tests/service_jwt_grant_admin_test.rs`（共享 MySQL，真实 HTTP oneshot
直连 token-service 内部路由——不经网关即被拒，证明检查在服务内部生效）：

| 测试 | 覆盖 | 结果 |
|---|---|---|
| `non_admin_service_jwt_is_denied_on_every_admin_route` | sub=payment-service、aud=token-service 的普通服务 JWT 对 **5 条路由全部 403**；grant 表行数不变、目标行状态不变；变更类各落 1 行 denied 审计（共 3）、查询类不落、**零 success 审计** | ✅ |
| `changing_request_params_cannot_escalate_service_jwt` | 改 owner/audience 指向 system_managed 系统授权行与普通行，仍全部 403，数据零变更 | ✅ |
| `empty_admin_list_is_fail_closed` | 清单为空时连 sub=token-service（历史默认管理主体）也 403 | ✅ |
| `grant_admin_list_canonicalizes_grace_suffix` | 清单项 `token-service#old` 与 canonical sub 匹配放行（归一语义） | ✅ |
| `admin_subject_still_operates_after_p1_fix` | 清单内主体 GET 200 / POST 200（放行半边） | ✅ |
| `service_entry_requires_bearer` / `service_entry_rejects_wrong_audience_jwt` | 未认证 / wrong-aud 仍 401（既有认证语义未变） | ✅ |
| `web_entry_rejects_user_without_permission` 等 6 条 Web UI 测试 | user-jwt 权限码门**原样有效**（未削弱） | ✅ |

### 配置 / 文档同步

- `config.rs`：`service_jwt_grant_admins` 字段（serde default 空）+ env
  overlay `TOKEN_SERVICE_JWT_GRANT_ADMINS` + `parse_grant_admins`（trim /
  去空 / `#old` 归一 / 去重）+ 5 个单元测试。
- `services/token-service/AGENTS.md`：可选 env 表新行 + 端点域 3 行更新 +
  P1 修复说明块。
- `services/token-service/README.md`：yaml 示例 + 配置说明表新行。
- 设计文档：`docs/superpowers/specs/2026-11-27-service-jwt-grant-admin-02-design.md`。

## C. P1-02 修复说明：业务变更与审计写入原子性

### 事务实现方式

`libs/common-db` 原有一个**无法构造的死代码** `Transaction`（Option-struct，
无 begin 入口、全仓库零使用点——已用 grep 验证）。本次将其激活：

- 重构为 enum（`Transaction::Mysql/Postgres/Sqlite`），补
  `DbPool::begin() -> Result<Transaction<'_>, sqlx::Error>`、
  `Transaction::commit()` / `rollback()`（错误类型 `sqlx::Error`，与宏族一致）。
- 新增事务宏族 `db_tx_execute!` / `db_tx_fetch_optional!` / `db_tx_fetch_scalar!`
  / `db_tx_fetch_all!`（`libs/common-db/src/query.rs`）：与池级 `db_*!` 宏
  **同一套 render + bind 计数校验**，同一份 SQL 源码按方言改写占位符
  （MySQL `?` / PG `$N`），executor 为 `&mut **t`（`&mut DB::Connection`）。
- token-service `models/service_jwt_grant.rs` 新增事务版：
  `find_by_owner_and_audience_for_update`（`SELECT ... FOR UPDATE`，双方言
  一致语法）、`create_grant_tx`、`set_grant_status_tx`、`delete_grant_tx`、
  `record_audit_tx`。
- **MySQL 与 PostgreSQL 走同一份宏渲染 SQL 与同一套事务 API**，语义一致。

### 事务边界

每个变更操作 = **单事务 = 业务变更 + success 审计行**：

| 操作 | 事务内步骤 | 失败语义 |
|---|---|---|
| CREATE | INSERT grant → INSERT audit(success) → commit | 审计失败 → 回滚（grant 不存在）→ 500 `"audit write failed"`；PK 冲突 → 回滚 → 409 |
| ENABLE/DISABLE | SELECT FOR UPDATE（锁下判 system_managed / 幂等）→ 条件 UPDATE → INSERT audit(success) → commit | 审计失败 → 回滚（状态原值）→ 500 |
| DELETE | SELECT FOR UPDATE → DELETE → INSERT audit(success) → commit | 审计失败 → 回滚（grant 仍在）→ 500 |

- 先查后改的 TOCTOU 窗口由 `FOR UPDATE` 行锁闭合（system_managed 判定在锁下）；
  system_managed 仍由条件 UPDATE/DELETE 在 SQL 内原子兜底（语义未放宽）。
- 显式 rollback + drop 兜底（sqlx 未提交事务归还连接时回滚），不吞审计异常：
  审计错误显式映射 500 并回滚。
- denied / 404 / 权限拒绝路径无业务变更：事务回滚后用池级 best-effort 审计
  （与 ADMIN-01 既有语义一致）；变更类 denied 审计写失败仍 fail-closed 500。
- 未采用"先提交业务、再补审计"的伪原子方案；未削弱任何一致性要求。

### 原子性证明：故障注入测试（真实数据库终态）

`tests/service_jwt_grant_atomicity_test.rs`——注入方式为
`ALTER TABLE token_service_jwt_grant_audit RENAME TO <唯一备份名>`（双方言
一致的 DDL）使事务内审计 INSERT 必然失败，断言**真实数据库终态**而非仅
HTTP 状态码，然后还原（每测试独立数据库，注入不外溢）：

| 用例 | 注入 | 数据库终态断言 | MySQL | PG |
|---|---|---|---|---|
| `*_create_rolls_back_when_audit_insert_fails` | 审计表改名 | grant **不存在** + 0 行 success 审计 + 响应体逐字 `"audit write failed"` | ✅ | ✅ |
| `*_update_rolls_back_when_audit_insert_fails` | 同上 | 状态保持 `active` + 0 行 success 审计 | ✅ | ✅ |
| `*_delete_rolls_back_when_audit_insert_fails` | 同上 | grant **仍存在** + 0 行 success 审计 | ✅ | ✅ |
| `*_success_path_audits_full_context` | 无注入 | create/disable/enable/delete 四次操作与审计**同生共死**：逐字段断言 actor_id/actor_type/before/after/action/request_id（客户端 UUID 原样落库） | ✅ | ✅ |
| `*_concurrent_disable_yields_single_success_audit` | 8 并发 disable | 全部 200（1 真写 + 7 幂等）、success 审计**恰 1 行**、终态 disabled | ✅ | ✅ |
| `*_concurrent_opposing_toggles_match_last_audit` | 10 并发对抗启停 | 每条翻转审计 after 与 action 匹配；**最终状态 == 最后一条 success 审计的 after**（业务与审计不可分离） | ✅ | ✅ |
| `*_concurrent_delete_yields_single_success_audit` | 6 并发 delete | 恰 1 个 204 + 5 个 404、success delete 审计恰 1 行、行消失 | ✅ | ✅ |
| `*_concurrent_system_managed_disables_all_denied` | 8 并发 disable 系统行 | 全部 409、行 `active`+`system_managed=true` 不动、零 success 审计 | ✅ | ✅ |

### MySQL / PostgreSQL 一致性证明

- 16 个用例（8 共享逻辑 × 2 方言）在两个方言上**全部通过**（同一次运行，
  见 §D）：事务边界、FOR UPDATE 行锁、条件写保护、审计字段在双方言表现一致。
- PG 走 `TEST_PG_URL` 外部容器模式（`postgres:16`，`pg_create...` 等用例实际
  执行，非 skip——本次运行输出中 16/16 ok）。

## D. 回归测试（逐项命令与结果）

环境：Windows host（MSVC），共享 MySQL `127.0.0.1:33306`，PG 容器
`artifact-dev-pg`（`postgres://postgres:artifact_dev@127.0.0.1:16500`）。

| # | 命令 | 结果 |
|---|---|---|
| 1 | `cargo check -p digua_common_db --features mysql,postgres` | ✅（仅既有 `init_pool_with` unreachable-pattern warning，stash 基线验证为既有） |
| 2 | `cargo check --workspace` | ✅（仅既有 `unused dependency async-trait` warning） |
| 3 | `cargo fmt -p digua_common_db -p token_service` | ✅ 无 diff |
| 4 | `cargo clippy -p digua_common_db -p token_service --features mysql,postgres --all-targets` | ✅ 0 error；仅 1 个既有 warning（同 #1，非本次引入） |
| 5 | `TEST_MYSQL_HOST=127.0.0.1 TEST_MYSQL_PORT=33306 cargo test -p token_service`（default，全量） | ✅ **27 个 test target 全绿 0 failed**（含单测 94、grant admin 30、atomicity 8） |
| 6 | `TEST_MYSQL_HOST=... TEST_MYSQL_PORT=... TEST_PG_URL=... cargo test -p token_service --features postgres`（方言全量） | ✅ **27 个 test target 全绿 0 failed**（含 pg_roundtrip 12、pg_grant_unique 9、PG atomicity 8/8） |
| 7 | P1-01 专项：`cargo test -p token_service --test service_jwt_grant_admin_test` | ✅ 30/30（含 6 条新增 P1-01 测试） |
| 8 | P1-02 专项（双方言）：`cargo test -p token_service --test service_jwt_grant_atomicity_test --features mysql,postgres` | ✅ 16/16 |
| 9 | 授权回归：`bootstrap_token_admin/web_test`、`api_key_auth_test`、`resource_token_auth_test`、`service_jwt_grant_test`、`service_jwt_test` | ✅ 全绿（12+14+5+3+…） |
| 10 | OpenAPI：`node scripts/dump-openapi-specs.js` 重生成 → 二次 dump 无新 diff（drift 门） | ✅ |
| 11 | `node scripts/check-service-standard.js` | ✅ 通过 |
| 12 | `node scripts/check-spec.js` | ✅ 通过 |
| 13 | `node scripts/contracts-gen.js --check` | ✅ Contracts in sync |
| 14 | `node scripts/check-db-naming.js` | ✅ PASS |

无失败项。（GitLab CI 的 `cargo test --workspace` + E2E 属推送后 CI 行为，
本分支未推送远端，见 §E。）

## E. 剩余风险（如实陈述）

1. **PG 测试依赖外部容器**：PG 用例（含 atomicity PG 半边）需要
   `TEST_PG_URL`；本机已用 `postgres:16` 容器实测 16/16 通过，但 CI 环境
   若未提供该变量，PG 用例按既有约定 `[skip]`（与 pg_roundtrip_test 等既有
   测试一致）。MySQL 用例无此依赖。
2. **同级入口未在本任务范围**：`/admin/bootstrap-tokens*`、`/admin/keys*`、
   `/admin/oauth-clients*`、`/token/internal/*` 等其他 service-jwt 入口仍是
   仅 `aud=token-service` 准入（与本修复前 grant 管理面同型风险）。任务书
   限定只修 grant 管理入口且禁止重构无关服务——建议后续任务按同一
   管理员清单模式收敛。
3. **管理员清单的运维治理**：清单由部署配置（env/yaml）管理，增删管理员
   需要变更部署配置并重启服务；配置本身无变更审计（与
   `SERVICE_BOOTSTRAP_TOKENS` 同级信任模型）。
4. **未推送远端 / 未跑 GitLab CI**：按任务书要求仅本地提交，GitLab CI
   `test` 阶段（workspace test + E2E + 门禁）待推送后触发；本报告 §D 的
   门禁命令已本地逐项执行通过。
5. **create 成功后 commit 前的响应构造**：commit 后有一次池级重查构造 DTO，
   若该查询失败会返回 500 但业务已提交（与 ADMIN-01 既有行为一致，属
   "响应不可靠"而非"数据不一致"；业务+审计仍原子）。

## F. 最终结论

**READY FOR INDEPENDENT SECURITY REVIEW**

- P1-01：管理权限与普通访问权限已分离，5 条管理路由全部前置管理员清单
  门（空清单 fail-closed），拒绝证据与放行证据均有真实 DB 状态断言的
  集成测试覆盖；Web UI 权限码机制未削弱。
- P1-02：CREATE/ENABLE/DISABLE/DELETE 与对应 success 审计在单事务提交，
  故障注入（审计表不可写）在 MySQL 与 PostgreSQL 上均证明业务变更回滚、
  数据库终态一致；并发用例证明行锁串行化下 success 审计与最终状态一一对应。
- 格式 / Clippy / 全量回归（default + postgres feature）/ 5 项 CI 门禁全部
  通过；无未验证事项被描述为通过（见 §E 如实清单）。

等待独立安全审查；本分支未合并 PR #19、未推送远端、未执行生产迁移、
未发布任何包或镜像。
