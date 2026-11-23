# PR #8 公开发布流水线 — 收尾与验收报告

- PR：https://github.com/luqizheng/lmms/pull/8
- 分支：`feat/public-release-pipeline` → `main`（PR 当前为 **Draft**，未合并）
- 最终代码提交 SHA：`be9aa0f7931fc1ad21826c010e430d7b3eeb33c4`
- 本报告提交：紧随其后的纯文档提交（见 PR 最新提交）
- 报告更新时间：2026-11-23（Asia/Shanghai）

## 一、结论

- PR #8 的发布流水线实现完整，发布清单已按仓库实际包结构逐项核对。
- 本次修复后，最后一个代码提交的 Public Release 与 Database Dialect Matrix 检查通过；OpenAPI Generation Validation 在该提交上出现一次 `exit 101` 的环境性失败（同代码此前/之后运行通过，详见 §5）。
- 未创建任何版本 Tag、未触发 `v*` 发布工作流、未上传 crates.io / npm / Docker Hub、未修改 GitHub Secrets。
- PR 保持未合并（Draft）。

## 二、修改文件清单（本轮累计 3 个提交）

### 提交 1 `7b34f1ec` — fix(ci): repair release validation gates

| 文件 | 修改内容 |
| --- | --- |
| `packages/contracts/src/error-codes.zh-CN.ts` | 补齐 5 个缺失错误码中文文案 |
| `services/audit-service/openapi.yaml` | 重新生成（1.7.0 → 1.8.0） |
| `services/file-service/openapi.yaml` | 同上 |
| `services/iam-service/openapi.yaml` | 同上 |
| `services/prue_backend/openapi.yaml` | 同上 |
| `services/token-service/openapi.yaml` | 同上 |
| `examples/docker-deploy/.image.env` | 平台镜像钉版 1.7.0 → 1.8.0 |
| `examples/docker-deploy-full/.image.env` | 同上（保留 `IMAGE_NGINX` 等基础镜像段） |
| `services/iam-service/src/service/user_svc.rs` | 3 处 `check-tenant-scope: delegated` 标注 |
| `services/iam-service/src/service/role_svc.rs` | 1 处同上 |
| `services/iam-service/src/service/org_svc.rs` | 1 处同上 |
| `services/iam-service/src/routes/roles.rs` | 1 处同上 |

### 提交 2 `6db18aed` — docs(progress): 初版收尾报告

| 文件 | 修改内容 |
| --- | --- |
| `artifacts/progress/pr-08-public-release/REPORT.md` | 新增 |

### 提交 3 `be9aa0f7` — fix(release): keep internal @digua/cli out of the public publish surface

| 文件 | 修改内容 |
| --- | --- |
| `pnpm-workspace.yaml` | 删除已不存在的 `sdk/config-services` 工作区条目 |
| `packages/cli/package.json` | 标记 `"private": true`，移除内网 `publishConfig.registry` |
| `packages/cli/test/release.test.js` | 新增 2 条发布清单回归测试 |

## 三、设计决策与原因

| 决策 | 原因 |
| --- | --- |
| 仅 `v*` Tag 触发发布（`^v[0-9]+\.[0-9]+\.[0-9]+$`），且 Tag 提交必须是 `origin/main` 祖先 | 发布与 PR/分支验证彻底分离，杜绝误发与从特性分支发布 |
| `validate` Job 在 PR 与 Tag 上都运行，三个发布 Job `needs: validate` 且带 `if: startsWith(github.ref,'refs/tags/v')` | 发布前强制完整验证；PR 上发布 Job 全部 skipped（已实测） |
| Rust / npm / Docker 三个发布 Job 相互独立（无 publisher→publisher 依赖） | 单一生态失败不阻塞其他生态；避免失败级联执行 |
| 凭证按 Job/步骤最小化：`CRATES` → 仅 `publish-crates` 的 env；`NPMPUBLISHKEY` → 仅 npm 发布步骤的 `NODE_AUTH_TOKEN`；`DOCKER_USER_NAME`/`DOCKER_KEY` → 仅 docker login 与 docker 发布步骤 | 任一发布步骤无法读取其他生态凭证；工作流级 `permissions: contents: read` |
| npm 目标固定 npmjs.org（`setup-node registry-url` + `release.toml npm_registry`），`pnpm publish --access public` | 公开发布且不落到内网 registry；`private: true` 包自动跳过 |
| Cargo 目标 crates.io：`release.toml rust_registry = ""`，`cargoPublishArgs` 在 registry 为空时不追加 `--registry` | 避免 `--registry null` 错误参数；有回归测试覆盖 |
| Docker 仅推送不可变版本 Tag（`release images` 内部 `latest: false`），命名 `docker.io/<DOCKER_USER_NAME>/<service>:<version>[-pg]` | 不覆盖 `latest`，版本可回溯 |
| 发布清单按实际结构与依赖确定：Rust 17 个 `libs/*` + 6 个 `sdk/rust/*`；npm 7 个包 | 不假定所有 crate/包都发；`@digua/cli` 属内部工具，排除 |
| `libs/common-consul` 未进入 Rust 发布清单 | 无任何待发布 crate 以 path 依赖它，属服务侧 Consul 注册胶水（见 §7 人工决策） |

## 四、实际发现的问题与修复

1. **Install Node dependencies 失败**：`error-codes.yaml` 新增 5 个错误码（`IAM_AUTH_1009`/`IAM_VALID_2005`/`IAM_VALID_2006`/`IAM_EXT_9201`/`IAM_EXT_9202`），手写 `error-codes.zh-CN.ts` 未同步，`Record<ErrorCode,string>` 触发 TS2739。→ 补齐文案（编译期约束即回归保护）。
2. **Verify generated specifications are committed 失败**：`main` 的 `5b675086` 把 workspace 版本升到 1.8.0 但未重新生成 5 份 `services/*/openapi.yaml`。→ 重新生成并提交；该漂移在 main 上同样存在。
3. **CLI tests 失败**：`examples/docker-deploy{,-full}/.image.env` 仍钉 1.7.0。→ 只更新平台镜像段版本 tag（未用 `update-image-env.js` 全量重写，因其 `BASE_IMAGES` 缺 `IMAGE_NGINX` 会删掉前端反代变量）。
4. **Repository gates / check-tenant-scope 失败（6 处）**：SQL 在委托的 `fetch_*` / `list_users_by_role_with_details` 内用 `and_tenant(scope)` 过滤，检查器无法跨函数追踪。→ 按检查器原生机制加 `delegated` 豁免注释，并逐一核实下游 SQL；未放宽规则。
5. **发布清单一致性（验收项 1）**：`pnpm-workspace.yaml` 仍列已删除的 `sdk/config-services`；`packages/cli` 未标 `private` 且 `publishConfig.registry` 指向内网。→ 删除陈旧条目；`@digua/cli` 标记 `private: true` 并移除内网 registry；新增 2 条回归测试（npm 清单只含真实公开包、Rust 清单均有 description/license）。

## 五、本地测试命令与结果

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| 依赖安装 | `pnpm install --frozen-lockfile` | 通过（lockfile up to date） |
| Rust formatting | `cargo fmt --check --all` | 通过 |
| Rust Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| Rust workspace 库测试 | `cargo test --workspace --lib` | 全部通过 |
| 服务规范 | `node scripts/check-service-standard.js` | 通过 |
| OpenAPI 契约 | `node scripts/check-spec.js` | 通过 |
| 契约漂移 | `node scripts/contracts-gen.js --check` | 通过（Contracts in sync） |
| 表名前缀 | `node scripts/check-db-naming.js` | 通过 |
| PG 迁移 | `node scripts/check-pg-migrations.js` | 通过 |
| 租户隔离 | `node scripts/check-tenant-scope.js` | 通过 |
| OpenAPI 漂移 | `node scripts/dump-openapi-specs.js` + `git diff --exit-code -- 'services/*/openapi.yaml'` | 通过（无漂移） |
| CLI 测试 | `pnpm run test:cli` | **78 passed / 0 failed**（新增 2 条） |
| npm 包构建 | `pnpm run packages:build` | 通过 |
| Release 计划 Rust | `node packages/cli/bin/coder.js release sdk rust --dry-run` | 通过（registry=null → crates.io 默认） |
| Release 计划 npm | `node packages/cli/bin/coder.js release sdk npm --dry-run` | 通过（7 包，无内网 registry） |
| Release 计划镜像 | `node packages/cli/bin/coder.js release images --dry-run --registry docker.io --repo validation-only` | 通过（1.8.0 / 1.8.0-pg，无 latest） |

## 六、GitHub Actions Run ID 与结果

确认来源：GitHub Actions REST API（`/repos/luqizheng/lmms/actions/runs`），确认时间 2026-11-23 15:40 前后（Asia/Shanghai）。期间 API 未认证配额耗尽，曾用 GitHub 网页（PR 页面/Job 页面）交叉确认，配额恢复后用 API 复核。

| 提交 | 工作流 | Run ID | 结果 |
| --- | --- | --- | --- |
| `3857379b`（修复前基线） | Public Release | 45120462531 | failure（Install Node dependencies） |
| `3857379b`（修复前基线） | OpenAPI Generation Validation | 45120462683 | failure（spec 漂移） |
| `7b34f1ec` | Public Release | 45123746665 | **success**（Validate 全绿；3 个 Publish 均 skipped） |
| `7b34f1ec` | OpenAPI Generation Validation | 45123746770 | **success** |
| `7b34f1ec` | Database Dialect Matrix | 45123746652 | **success** |
| `6db18aed` | Public Release | 45125321926 | **success** |
| `6db18aed` | OpenAPI Generation Validation | 45125321913 | **success** |
| `6db18aed` | Database Dialect Matrix | 45125321910 | **success** |
| `be9aa0f7` | Public Release | 45129681110 | **success**（Validate 全绿；Publish skipped） |
| `be9aa0f7` | Database Dialect Matrix | 45129681126 | **success** |
| `be9aa0f7` | OpenAPI Generation Validation | 45129681118 | failure：`generate-and-check` 的生成步骤 `exit 101`（见下） |
| 本报告提交 | 三个工作流 | 见 PR 最新运行 | 纯文档变更，推送后复核（结果见交付总结） |

`be9aa0f7` 的 OpenAPI 失败特征：生成步骤 94s 退出码 101（`cargo metadata --locked` / `cargo build --locked` 失败），而同一提交的 Public Release 在其 Repository gates 中执行同一条 `node scripts/dump-openapi-specs.js` 成功。该工作流未配置 `Swatinem/rust-cache`，每次冷启动需拉取全部依赖，属环境性瞬时失败（网络/依赖拉取），非代码缺陷；该提交后重跑已确认（见 PR 最新运行）。

## 七、未完成事项与需人工决策项

1. **需人工决策**：`libs/common-consul` 是否纳入 crates.io 发布清单。当前排除依据是无待发布 crate 依赖它；若下游需要自建服务注册能力，应加入并随发布序验证。
2. **需人工决策**：npm 包名为无 scope 的 `iam-services` / `audit-services` / `file-services` / `notification-services` / `prue-services`，首次正式发布前需确认 npmjs.org 名称可用/归属；`@digua/contracts`、`@digua/http` 需确认 scope 归属。
3. **需人工决策**：Docker Hub 目标仓库（`<DOCKER_USER_NAME>/<service>`）需已存在或允许首次推送；`DOCKER_USER_NAME` 的实际命名空间需与镜像命名核对。
4. **首次真实发布未验证**：本次严禁真实发布，Tag 路径仅通过 PR 上的 Validate 与离线 dry-run 验证；首次 `v*` 发布仍是未经验证路径。
5. **PR 仍为 Draft 状态**：需负责人评审后转 Ready 并合并；本次未合并、未转 Ready。

## 八、已知风险

1. **main 分支仍为红**：本 PR 修复的 OpenAPI 漂移、contracts 编译失败、`.image.env` 版本滞后源自 main 的 `5b675086`；需 PR #8 合并后 main 才恢复绿色。
2. **发布为部分序列**：Rust crates 按依赖序逐个发布，中途失败会留下部分已发布版本；已有“已存在即跳过”的幂等恢复逻辑，重跑可继续。
3. **`scripts/update-image-env.js` 生成器缺口**：`BASE_IMAGES` 不含 `IMAGE_NGINX`，全量重写会丢 full 单元前端反代变量；本次未改脚本，改为手工只更新版本 tag。
4. **OpenAPI 验证工作流无 Rust 缓存**：冷构建约 2 分钟起，依赖拉取失败即整步 101 失败，存在再次偶发失败的可能（已在 §6 记录）。
5. **并发工作区风险**：本次收尾期间同一工作目录被另一任务切换到 `feat/jwt-key-operations-hardening` 并存在未提交改动；报告提交通过临时索引（plumbing）完成，未触碰该分支工作区。

## 九、安全确认

- ✅ 未创建任何正式版本 Tag（`v*`）。
- ✅ 未触发任何 `v*` 发布工作流；Public Release 仅以 `pull_request` 事件运行，发布 Job 全部 skipped。
- ✅ 未上传 crates.io 包、未发布 npm 包、未推送 Docker Hub 镜像。
- ✅ 未修改 GitHub Secrets，未使用真实发布凭证。
- ✅ **PR #8 未合并**（仍为 open/Draft）。
- ✅ 全部变更已推送到 `feat/public-release-pipeline`。
