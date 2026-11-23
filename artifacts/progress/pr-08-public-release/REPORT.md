# PR #8 公开发布流水线 — 收尾报告

- PR：https://github.com/luqizheng/lmms/pull/8
- 分支：`feat/public-release-pipeline` → `main`
- 最终提交 SHA：`7b34f1ecbb79e7823cb32df355bdfbac40f993b1`
- 报告生成时间：2026-11-23（Asia/Shanghai）
- PR 状态：**open（未合并）**，`mergeable=true`，`mergeable_state=clean`

## 一、结论

- PR #8 代码实现完整（Tag 触发发布、验证与发布分离、三类发布独立 Job、凭证仅在发布步骤使用等既定设计均保留）。
- 本次提交后 GitHub Actions 三个工作流全部通过。
- 未创建任何版本 Tag、未触发 `v*` 发布工作流、未上传 crates.io / npm / Docker Hub、未修改 GitHub Secrets。
- PR 保持未合并。

## 二、本次修改文件清单

| 文件 | 修改内容 |
| --- | --- |
| `packages/contracts/src/error-codes.zh-CN.ts` | 补齐 5 个缺失错误码中文文案 |
| `services/audit-service/openapi.yaml` | 重新生成（版本 1.7.0 → 1.8.0） |
| `services/file-service/openapi.yaml` | 同上 |
| `services/iam-service/openapi.yaml` | 同上 |
| `services/prue_backend/openapi.yaml` | 同上 |
| `services/token-service/openapi.yaml` | 同上 |
| `examples/docker-deploy/.image.env` | 平台镜像段钉版 1.7.0 → 1.8.0（基础镜像段未动） |
| `examples/docker-deploy-full/.image.env` | 同上（保留 `IMAGE_NGINX` 等基础镜像段） |
| `services/iam-service/src/service/user_svc.rs` | 3 处 `check-tenant-scope: delegated` 标注（get / update / batch_get_by_ids） |
| `services/iam-service/src/service/role_svc.rs` | 1 处 `check-tenant-scope: delegated` 标注（get） |
| `services/iam-service/src/service/org_svc.rs` | 1 处 `check-tenant-scope: delegated` 标注（get） |
| `services/iam-service/src/routes/roles.rs` | 1 处 `check-tenant-scope: delegated` 标注（list_users_by_role） |
| `artifacts/progress/pr-08-public-release/REPORT.md` | 本报告 |

## 三、修复的问题及原因

### 1. `Install Node dependencies` 失败（Public Release / Validate release candidate）

原因：`contracts/error-codes.yaml` 新增 5 个错误码（`IAM_AUTH_1009`、`IAM_VALID_2005`、`IAM_VALID_2006`、`IAM_EXT_9201`、`IAM_EXT_9202`），但手写的 `packages/contracts/src/error-codes.zh-CN.ts` 未同步；该文件类型为 `Record<ErrorCode, string>`，`tsc` 在 `prepare` 阶段报 TS2739，导致 `pnpm install --frozen-lockfile` 失败。

修复：补齐对应中文文案。编译期 `Record` 约束即为回归保护，无需新增运行时测试。

### 2. `Verify generated specifications are committed` 失败（OpenAPI Generation Validation）

原因：`main` 的 `chore(release): bump workspace version to 1.8.0` 只改了 `Cargo.toml` / `Cargo.lock`，未重新生成 `services/*/openapi.yaml`，5 份 spec 仍写 1.7.0 → CI 生成后出现版本漂移。

修复：本地 `node scripts/dump-openapi-specs.js` 重新生成并提交 5 份 spec。该提交同时修复了 main 上同源的失败（main 的 run 45115143590 亦为 failure）。

### 3. `CLI tests` 失败

原因：`examples/docker-deploy{,-full}/.image.env` 仍钉 1.7.0，而 CLI 测试断言部署单元钉版必须等于服务版本（1.8.0）。

修复：仅更新两个文件的平台镜像段 tag 到 1.8.0。未使用 `scripts/update-image-env.js` 全量重写，因为该脚本的 `BASE_IMAGES` 未包含 `IMAGE_NGINX`，全量重写会删掉 full 单元的前端反代变量（见风险 §6.3）。

### 4. `Repository gates` 中 `check-tenant-scope` 失败（6 处）

原因：这 6 个函数本身不含 SQL，租户过滤发生在委托的 `fetch_*` / `list_users_by_role_with_details` 中（均使用 `qb.and_tenant(scope)` 或 `u.tenant_id = ?`）。检查器基于函数体正则匹配，无法跨函数追踪，属误报。

修复：使用检查器原生支持的 `// check-tenant-scope: delegated` 豁免注释标注，并逐一核实下游 SQL 确有租户过滤。未修改检查器、未放宽规则。

## 四、本地执行的测试与结果

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| 依赖安装 | `pnpm install --frozen-lockfile` | 通过 |
| Rust formatting | `cargo fmt --check --all` | 通过 |
| Rust Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| Rust workspace 库测试 | `cargo test --workspace --lib` | 全部通过 |
| 服务规范 | `node scripts/check-service-standard.js` | 通过 |
| OpenAPI 契约 | `node scripts/check-spec.js` | 通过（summary 缺失为既有 warn） |
| 契约漂移 | `node scripts/contracts-gen.js --check` | 通过（Contracts in sync） |
| 表名前缀 | `node scripts/check-db-naming.js` | 通过 |
| PG 迁移 | `node scripts/check-pg-migrations.js` | 通过 |
| 租户隔离 | `node scripts/check-tenant-scope.js` | 通过 |
| OpenAPI 漂移 | `node scripts/dump-openapi-specs.js` + `git diff --exit-code -- 'services/*/openapi.yaml'` | 通过（无漂移） |
| CLI 测试 | `pnpm run test:cli` | 76 passed / 0 failed |
| npm 包构建 | `pnpm run packages:build` | 通过 |
| Release 计划（Rust） | `node packages/cli/bin/coder.js release sdk rust --dry-run` | 通过（23 个 crate 计划，registry=null → crates.io 默认源） |
| Release 计划（npm） | `node packages/cli/bin/coder.js release sdk npm --dry-run` | 通过（7 个包，0.1.0） |
| Release 计划（镜像） | `node packages/cli/bin/coder.js release images --dry-run --registry docker.io --repo validation-only` | 通过（版本 1.8.0，pg 方言，两批次） |

## 五、GitHub Actions 运行结果（最终提交 `7b34f1ec`）

| 工作流 | Run ID | 结果 |
| --- | --- | --- |
| Public Release | [45123746665](https://github.com/luqizheng/lmms/actions/runs/45123746665) | success（Validate release candidate 通过；3 个 Publish Job 均 skipped） |
| OpenAPI Generation Validation | [45123746770](https://github.com/luqizheng/lmms/actions/runs/45123746770) | success |
| Database Dialect Matrix | [45123746652](https://github.com/luqizheng/lmms/actions/runs/45123746652) | success |

修复前同一分支的最新失败运行（作为对照）：Public Release [45120462531](https://github.com/luqizheng/lmms/actions/runs/45120462531) failure、OpenAPI Generation Validation [45120462683](https://github.com/luqizheng/lmms/actions/runs/45120462683) failure。

Public Release 关键步骤逐项确认：Rust formatting、Rust Clippy、Rust workspace library tests、Repository gates、CLI tests、Build public npm packages、Validate release plans without publishing 全部 success。

## 六、未完成事项与风险

1. **未执行真实发布**：crates.io / npmjs.org / Docker Hub 均未上传；Tag 触发的发布路径仅通过 dry-run 与工作流静态结构验证，真实发布仍待打 `v*` Tag 后的首次执行验证。
2. **main 仍为红**：本 PR 修复的 OpenAPI 漂移、contracts 编译失败、`.image.env` 版本滞后均源自 main 的 `5b675086` 版本升级提交未配套更新；需 PR #8 合入后 main 才会恢复绿色。
3. **`scripts/update-image-env.js` 生成器缺口**：其 `BASE_IMAGES` 未包含 `IMAGE_NGINX`，全量重写 `examples/docker-deploy-full/.image.env` 会丢失前端反代变量。本次未改该脚本（超出 PR 范围），已改为手工只更新版本 tag。
4. **管理员合并权限**：PR 未合并，等待人工评审与合并。
5. 工作区存在未跟踪文件 `rust-ci-timings.json`（非本次任务产物），未纳入提交。

## 七、明确确认

- ✅ 未创建任何正式版本 Tag（`v*`）。
- ✅ 未触发任何 `v*` 发布工作流；Public Release 仅以 `pull_request` 事件运行，发布 Job 全部 skipped。
- ✅ 未上传 crates.io 包、未发布 npm 包、未推送 Docker Hub 镜像。
- ✅ 未修改 GitHub Secrets。
- ✅ 未使用任何真实发布凭证。
- ✅ **PR #8 未合并**（state = open）。
- ✅ 全部变更已推送到 `feat/public-release-pipeline`（`3857379b..7b34f1ec`）。
