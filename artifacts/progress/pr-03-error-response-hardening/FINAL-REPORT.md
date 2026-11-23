# PR #3 最终交付报告 — MT-03 Error Response Hardening

**日期：** 2026-11-23
**仓库：** `github.com/luqizheng/lmms`
**PR：** [#3](https://github.com/luqizheng/lmms/pull/3)（`mt-03-error-response-hardening` → `main`，OPEN）

## 1. Rebase 与 HEAD 记录

| 项 | SHA | 说明 |
| --- | --- | --- |
| Rebase 前 PR HEAD | `46e1c97afe08faa768531432f6e238d38d5f96ca` | 原始 8 commit |
| Rebase 前 main | `5b675086597e001e794a4c2f3a3575a431af6cc9` | v1.8.0 bump |
| 第一次 rebase 后 HEAD | `facd6291` | rebase 到 `5b675086`，无冲突，含测试补充 commit |
| 第二次 rebase（main 推进） | — | main 新增 `305fc2a2`（PR #9 合并），二次 rebase |
| Rebase 后 main 基点 | `305fc2a2e531589de899f726be849343f1b388a5` | 最新 main |
| 最终 PR HEAD（报告提交前） | `e5fca928` | 9 个 commit，无冲突 |
| 最终 PR HEAD（含本报告） | 见 PR 页面 / §2 | 本报告 commit 即分支末端 |

- 备份分支：`backup/mt-03-pre-rebase`（本地，指向 `46e1c97a`）。
- rebase 两次均无冲突；重放的 8 个历史 commit 内容未修改。

## 2. 修改文件清单与归因

任务书圈定的 4 个文件全部保留，另有两个 rebase 后新增文件，均由本功能直接引起：

| 文件 | 变更 | 归因 |
| --- | --- | --- |
| `.github/workflows/core-crates.yml` | 新增（17 行） | 本功能 CI 门禁（原 PR 内容） |
| `docs/architecture/MT-03-ERROR-RESPONSE-HARDENING-REPORT.md` | 新增（27 行） | 本功能架构记录（原 PR 内容） |
| `libs/common-error/Cargo.toml` | +2/-1 | 新增 tokio dev-dependency（原 PR 内容） |
| `libs/common-error/src/lib.rs` | +127/-15 | 脱敏实现 + 测试（原实现 + 本次补测） |
| `Cargo.lock` | +1 | **本次补充**：`digua_common_error` 解析依赖列表新增 `tokio`。由 Cargo.toml dev-dep 直接引起；CI（openapi-generation）以 `--locked` 运行，不同步提交必然失败。历史 HEAD（`46e1c97a` 及更早）的 CI 失败根因即此漏提交。 |
| `libs/common-error/src/lib.rs`（测试段） | 详见 §4 | **本次补充**：扩展测试覆盖至任务书全部 7 项 |

未修改任何无关服务；未修改 main。

## 3. 错误响应脱敏实现说明

核心是 `AppError::public_message()`（`libs/common-error/src/lib.rs:81`）+ `IntoResponse` 只序列化白名单字段：

- **响应体结构** `ErrBody { code, message, request_id }` —— 仅这三个字段进入 HTTP JSON，`source` 链与原始 `message`（Internal/External 场景）永不序列化。
- **`Internal` → 固定 `"Internal server error"`**：数据库连接串、用户名密码、SQL、底层 transport 详情全部被固定消息替换；原始消息仍保留在 `Display`（`[{code}] {message}`）与 `#[source]` 链中，服务端日志可诊断。
- **`External` → 固定 `"Upstream request failed"`**：上游 URL、query 参数（如 token）、hostname 全部不出现于响应。
- **`From<sqlx::Error>`**：`RowNotFound` → `404 DB_NOT_FOUND "row not found"`（客户端可操作）；其余 → `internal("DB_ERR", "database error")`（脱敏为固定消息，原文进 source 链）。
- **`From<reqwest::Error>`**：`external("HTTP_ERR", e.to_string())` —— 原始错误文本（可能含 URL）存入 message 供 Display 诊断，但 `External` 类型的 `public_message` 覆盖保证响应体只有固定消息。
- **状态码映射不变**：BadRequest→400、Unauthorized→401、Forbidden→403、NotFound→404、Conflict→409、TooManyRequests→429、Internal→500、External→502。
- 客户端可处理类型（BadRequest/Unauthorized/Forbidden/NotFound/Conflict/TooManyRequests）原消息原样保留。

## 4. 测试清单（`response_safety_tests`，全部断言真实 Axum `IntoResponse` JSON）

本次新增/增强（7 个测试）：

1. `internal_details_are_not_serialized_to_http_clients` — 增强：断言 500 + code + 固定消息 + **request_id 保留** + secret 不泄露。
2. `upstream_details_are_not_serialized_to_http_clients` — 增强：断言 502 + 固定消息 + secret 不泄露。
3. `reqwest_error_conversion_is_sanitized_in_http_response` — **新增**：真实离线 `reqwest::Error`（loopback 无监听端口，错误 Display 含完整 URL+query）经 `From` 转换后，响应脱敏。
4. `sqlx_error_conversion_is_sanitized_in_http_response` — **新增**：`sqlx::Error::Configuration`（含连接串 secret）经 `From` 转换后 500 + `DB_ERR` + 固定消息 + secret 不泄露。
5. `sqlx_row_not_found_keeps_client_actionable_message` — **新增**：`RowNotFound` → 404 + `DB_NOT_FOUND` + 消息保留。
6. `client_error_messages_remain_actionable_in_http_response` — 增强：断言 400 + code + 原消息保留。
7. `internal_details_remain_visible_via_display_and_source_chain` — **新增**：验证 `Display` 输出与 `source()` 链仍可取到内部详情（服务端诊断能力未受损）。

测试均通过 `to_bytes(...)` 读取实际 Axum response body 并反序列化为 JSON 断言，同时断言 `StatusCode` 不变。

## 5. 本地验证结果（Windows 主机，MSVC 工具链）

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | ✅ 通过（修掉一处多余空行） |
| `cargo test -p digua_common_error --lib` | ✅ 7 passed / 0 failed |
| `cargo clippy -p digua_common_error --all-targets -- -D warnings` | ✅ 通过（修掉一处 `clippy::err_expect`；workspace manifest 3 个 unused-dependency warning 为 main 既有，与本 PR 无关） |
| `cargo test --workspace --lib` | ✅ 31 个测试目标、**200 passed / 0 failed**（依赖共享 MySQL 127.0.0.1:33306 / Redis 127.0.0.1:36379，已按项目 `infrastructure/infra-mysql` 栈启动，未改动任何迁移与生产数据） |

### 历史 CI 问题核查（任务书 §四）

1. **`pg_migration_test.rs` 引用 config-service**：该文件现位于 `libs/common-db/tests/pg_migration_test.rs`，仅引用 file-service / prue_backend / audit-service 的 postgres 迁移（均存在），无 config-service 引用；无 `TEST_PG_URL` 时自动跳过。✅ 已解决。
2. **dialect-matrix package/feature 一致性**：workflow 引用的 7 个 package（`digua_common_db`、`audit_service`、`file_service`、`token_service`、`prue_backend`、`notification_service`、`iam_service`）全部在 workspace 成员内，所需 `mysql`/`sqlite`/`postgres` feature 在对应 Cargo.toml 均有定义。✅ 一致。

未删除、未跳过任何既有测试。

## 6. GitHub Actions 结果（HEAD `e5fca928`，2026-11-23 全部完成）

| Workflow | Run | Job 结果 |
| --- | --- | --- |
| Core Rust Crate Tests | [45148368723](https://github.com/luqizheng/lmms/actions/runs/45148368723) | `test-common-error` ✅ success（6 steps） |
| Database Dialect Matrix | [45148368878](https://github.com/luqizheng/lmms/actions/runs/45148368878) | `test-sqlite` ✅ / `test-mysql` ✅ / `test-postgres` ✅ / `test-services` ✅ / `test-services-postgres` ✅（各 7-9 steps 全过；`mssql`/`dameng` 为 workflow 显式 skip 项，不属支持方言） |
| OpenAPI Generation Validation | [45148368724](https://github.com/luqizheng/lmms/actions/runs/45148368724) | `generate-and-check` ✅ success（10 steps，含 `git diff` drift 检查） |

三个 workflow、全部 7 个实际执行的 job 均逐个核查为 success（非仅 workflow 级绿标）。

### CI 失败根因区分（历史/中间 HEAD → 修复 → 最终 HEAD）

- **历史分支遗留失败**：dialect-matrix @ `46e1c97a`（2026-11-22）的 `test-postgres` / `test-services` / `test-services-postgres` 失败，根因是当时 `pg_migration_test.rs` 仍 `include_str!` 已删除的 `services/config-service` 迁移文件（编译期 No such file）。rebase 后分支包含新 main 的已修复版本，该失败自然消失。
- **本次改动引起的失败**：`OpenAPI Generation Validation` @ `facd6291` 因 `Cargo.lock` 未随 dev-dep 同步（`--locked` 报错）。已通过 `e5fca928` 补提交修复。
- **main 既有问题**：main @ `5b675086` 的 openapi job 曾失败（openapi.yaml 生成漂移），随 PR #9 合并（`305fc2a2`）已消失；main @ `305fc2a2` openapi 成功。与本 PR 无关。
- **环境/服务问题**：内网 registry `zhcoder-docker-registry.com:8000` 本地不可达，本地测试改用 Docker Hub 官方镜像（仅本地 `.env`，不入库）；GitHub Actions 侧历史 run 有 rustup/registry 网络抖动重试（main 上同样存在），与本 PR 逻辑无关。

## 7. 剩余风险

- `From<reqwest::Error>` 仍把 `e.to_string()`（含上游 URL）存入 `message` 字段——响应层已脱敏，但该字段会进入服务端日志（`Display`）。这是有意设计（诊断需要），不构成泄露路径；如未来把 `Display` 输出回传客户端需保持 `public_message` 策略。
- `codes.rs` / 权限码等契约层未纳入本 PR 范围（`DB_ERR`/`HTTP_ERR` 等码为自由字符串，非 `contracts/error-codes.yaml` 管控码）。如需契约化可在后续任务处理，不阻塞合并。
- GitHub Actions 为共享 runner，历史 run 显示偶发网络抖动（rustup 安装重试）；与本 PR 逻辑无关。

## 8. 合并建议

**建议合并。** 前提确认：基于最新 main（`305fc2a2`）✅、本地三件套 + 全 workspace lib 测试通过 ✅、脱敏测试覆盖任务书全部 7 项 ✅、HEAD `e5fca928` 的必需 GitHub Actions 全 job 通过 ✅、无新增 P0/P1 安全问题 ✅。本报告已提交至 PR 分支（报告 commit 即分支末端，其自身触发的 CI 复跑仅含 docs 变更，与 `e5fca928` 的代码状态完全一致）。合并后可删除备份分支 `backup/mt-03-pre-rebase`（本地）。
