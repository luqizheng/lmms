# v1.8.0 发布与 PR 积压审计

- 审计时间：2026-11-23（Asia/Shanghai）
- 审计分支：PR #8 `feat/public-release-pipeline` @ `4558aea7`（`main` @ `5b675086` 基线）
- 边界声明：本批次**未**合并任何 PR、**未**创建/删除 tag、**未**向 crates.io / npmjs.org / Docker Hub 发布、**未**修改 GitHub Secrets、**未**删除任何包或镜像、**未**改动 `main`。

---

## 1. PR #8 是否达到合并条件

**未达到。** 仍为 Draft，且 GitHub 判定 `mergeable_state = unstable`（存在未完成的必需检查）。

已完成的发布前置加固（本批次提交 `4558aea7`）：

| # | 要求 | 处理 |
|---|---|---|
| 1 | 确认 npm 包名在 npmjs.org 可用 | 已核查（见表 §3），发现 3 个未 scoped 名称不可用 |
| 2 | 被占用名称统一改 `@digua/` 前缀 | 已改：5 个服务 SDK + 根 `package.json` 过滤命令 + 文档 |
| 3 | 确认 npm organization / 发布权限 | **未完成**（需登录 npm 核对 `@digua` org 归属） |
| 4 | license 不得用 UNLICENSED | 已改为 `Apache-2.0`（7 个公开包），并新增根 `LICENSE` |
| 5 | crates.io 名称/版本/依赖/权限 | 24 个 crate 名称全部可解析；23 个版本已存在，`digua_http_client` 首发布；owners=`luqizheng` |
| 6 | Docker Hub 用户/仓库/权限 | **未完成**（无法确认 `DOCKER_USER_NAME` namespace） |
| 7 | 凭证隔离 / 失败处理 / 幂等 / 断点续发 | 已确认：逐 job 仅注入自身 secret；npm/crates 已存在即跳过；Docker 重推覆盖同 tag |
| 8 | 增加 `workflow_dispatch` 指定已有 tag | 已加（必填 `tag` 输入） |
| 9 | 手动发布仍跑完整验证 + tag 属于 main | 已加：格式校验 + `git merge-base --is-ancestor` + 复用全套 validate job |
| 10 | 更新发布手册与测试 | 已更新 runbook；新增 5 个单测，`node --test` 83 passed |

合并前剩余阻断：

- CI 最新一轮（`4558aea7`）：Public Release 与 OpenAPI Generation Validation 已 **success**；Database Dialect Matrix 仍排队（无进展），需转绿后才能转 Ready。
- 上一提交 `be9aa0f7` 的 **OpenAPI Generation Validation 失败**（run `45129681118`）已被同一提交族的新一轮 success 证伪为环境抖动。
- `@digua` npm org 与 Docker Hub namespace 两项外部权限仍未验证。

---

## 2. v1.8.0 是否具备补发条件

**不具备。** 补发计划本身（dry-run）全部通过，但被外部条件阻断：

1. npm `@digua` organization 归属与发布 token 权限未验证。
2. Docker Hub 目标 namespace 未确认。
3. `digua_http_client@1.7.0` 在 crates.io 不存在且名称未占位（首次发布不可撤回）。
4. 公开分发缺少负责人书面批准。
5. 发布 workflow 尚未进入 `main`（PR #8 未合并）。

详见 `artifacts/progress/v1.8.0-public-release/INVENTORY.md` 与 `DRY-RUN.md`。

---

## 3. npm 包名可用性（registry.npmjs.org 实测）

| 名称 | 状态 | 结论 |
|---|---|---|
| @digua/contracts | 404 | 名称可用（scope 归属待验证） |
| @digua/http | 404 | 可用（同上） |
| @digua/iam-services | 404 | 可用（同上） |
| @digua/audit-services | 404 | 可用（同上） |
| @digua/file-services | 404 | 可用（同上） |
| @digua/notification-services | 404 | 可用（同上） |
| @digua/prue-services | 404 | 可用（同上） |
| ~~file-services~~ | **已被他人占用**（gopinathkaliappan 0.0.2） | 必须改名 → 已改 `@digua/file-services` |
| ~~notification-services~~ | **已被他人占用**（arachnomesh 1.2.0） | 必须改名 → 已改 `@digua/notification-services` |
| ~~iam-services~~ | 存在空壳记录、无任何版本 | 不可用 → 已改 `@digua/iam-services` |
| audit-services / prue-services | 404 | 原本可用，为一致性统一加 scope |

`npm search scope:digua` = 0 个公开包。

---

## 4. 许可证确认

| 包 | 旧值 | 现值 |
|---|---|---|
| @digua/contracts | UNLICENSED | Apache-2.0 |
| @digua/http | UNLICENSED | Apache-2.0 |
| @digua/{iam,audit,file,notification,prue}-services | UNLICENSED | Apache-2.0 |
| Rust crates（24） | Apache-2.0 | 不变 |
| 仓库根 LICENSE | 缺失 | 新增 Apache-2.0 |

`UNLICENSED` 不再作为公开授权依据。法务/负责人批准仍无记录。

---

## 5. crates.io 与 Docker Hub 权限检查

**crates.io**

- 抽查 owners：`digua_common_error` / `digua_common_consul` / `digua_jwt_ctx` / `digua_audit_sdk` → 均为 `luqizheng`。
- 已发布版本：libs 全部 1.6.0（consul / jwt 为 1.7.0）；sdk 1.6.0/1.7.0。
- 唯一缺口：`digua_http_client` 在 crates.io **不存在**（名称当前可用）。

**Docker Hub**

- 候选 namespace `luqizheng` / `digua` / `lmms` 均返回 0 个公开仓库。
- 工作流使用 `secrets.DOCKER_USER_NAME`，无法匿名确认其取值与权限。
- 结论：**未完成权限检查**。

---

## 6. PR #1–#7 真实状态

| PR | 状态 | main 实际落地 | 处置建议 |
|---|---|---|---|
| #1 | open | 0%（.cargo 仍含 kellnr；技能仍教私有 registry；iam 仍 `process::exit(0)`） | rebase 保留 |
| #2 | closed | 核心已落地（`DialectKind` 仅 3 种，无 dameng/mssql 实现） | 保持关闭 |
| #3 | open | 0%；`reqwest::Error` 明细仍直接序列化（真实泄露） | 优先合并 |
| #4 | open(draft) | OpenAPI 脚本已由 #6 取代；timings 仍缺失 | 收窄重开 |
| #5 | closed | 类型化限流错误、`--` 处理、OpenAPI 脚本已落地；timings/独立 Rust CI workflow 仍缺 | 保持关闭 |
| #6 | merged | 批量 OpenAPI 生成已生效 | — |
| #7 | merged | JWT 密钥持久化/轮换/备份恢复已生效（`keystore.rs` 974 行） | — |

专项回答：

- Dameng / MSSQL：不缺失（已按预期移除）。
- Rust CI 修复：部分缺失（workflow 与 timings）。
- PR #3 泄露风险：存在（上游 URL/query 可回显）。
- PR #1 模板：仍必要。
- PR #4 timings：未被替代。

详见 `artifacts/progress/pr-backlog-review/REPORT.md`。

---

## 7. PR #9 是否创建

**已存在，无需新建。** GitHub 上 `PR #9 feat(token): JWT key lifecycle operations hardening` 为 open（head `feat/jwt-key-operations-hardening` @ `39df6ce1`，18 files，+1742/-68）。

与 PR #7 基线的真实缺口（已在 main 代码中逐项验证）：

| 缺口 | PR #7 基线 | 证据 |
|---|---|---|
| 密钥生命周期审计 | 完全没有 | main 无 `keystore-audit` / 审计模块 |
| 备份完整性 | 裸 `KeyStoreData`，无校验和/格式版本 | main 无 `backup_format_version` / `keystore_sha256` |
| 恢复到 live 的保护 | 直接覆盖，无快照/回读/回滚 | main 无恢复前快照逻辑 |
| 并发/丢失更新 | `atomic_write` 不防丢失更新 | main 无 lock / `Stale` 指纹守卫 |

本地验证：`cargo test -p token_service --lib` → **66 passed / 0 failed**（含审计、并发、备份篡改、恢复回滚用例）。
结论：缺口真实，PR #9 不属于重复实现；保持独立分支、独立测试、独立报告。

---

## 8. CI 结果与 Run ID

| Run ID | Workflow | 分支 / SHA | 结果 |
|---|---|---|---|
| 45129681126 | Database Dialect Matrix | feat/public-release-pipeline / be9aa0f7 | 未完成（排队/进行中） |
| 45129681118 | OpenAPI Generation Validation | feat/public-release-pipeline / be9aa0f7 | **failure**（Generate all service OpenAPI specifications） |
| 45129681110 | Public Release | feat/public-release-pipeline / be9aa0f7 | success |
| 45130564019 | Public Release | feat/public-release-pipeline / 4558aea7 | **success** |
| 45130563878 | OpenAPI Generation Validation | feat/public-release-pipeline / 4558aea7 | **success**（07:25 完成，据此可判定 be9aa0f7 的失败为环境抖动） |
| 45130563947 | Database Dialect Matrix | feat/public-release-pipeline / 4558aea7 | 排队中（创建后无进展，疑似无可用 runner） |
| 45130581838 | OpenAPI Generation Validation | feat/jwt-key-operations-hardening / 39df6ce1 | **success** |
| 45130581924 | Database Dialect Matrix | feat/jwt-key-operations-hardening / 39df6ce1 | 排队中 |

本地：`node --test packages/cli/test/*.test.js` → 83 passed；`cargo test -p token_service --lib` → 66 passed。

---

## 9. 未解决问题

1. `@digua` npm organization 归属与发布权限未验证。
2. Docker Hub namespace / `DOCKER_USER_NAME` 未确认。
3. `digua_http_client` 首次发布名称未占位。
4. 公开分发缺少负责人书面批准。
5. OpenAPI Generation Validation 在 `be9aa0f7` 失败原因未定位（疑似环境抖动，待新一轮结果）。
6. PR #8 仍为 Draft，需转 Ready 且 CI 全绿后才具备合并条件。
7. PR #1 / #3 需 rebase；PR #2 / #4 / #5 建议关闭或转为独立 issue。
8. crates.io 与 npm 的制品版本线（1.6/1.7 与 0.1.0）与平台 1.8.0 不一致，需负责人确认是否可接受。
