# PR #8 当前 head 复核 — Public Release Pipeline

- 复核时间：2026-11-23
- PR：`luqizheng/lmms#8`（`feat(release): add tag-triggered public publishing pipeline`），state=open
- 复核 head：`327b0016cf38ea665e0949d209b4a89834bbac2a`（`feat/public-release-pipeline`，已 fetch 至本地核对，非仅依据报告）
- base：`main` @ `5b675086`（= v1.8.0 tag）
- 只读复核：未执行任何真实发布、未动 tag/secret/main。

## 1. 完整 diff 是否仍包含 npm scope 改名与许可证修改 — 是

`git diff 5b675086..327b0016` 确认：

- **scope 改名**：5 个 `sdk/*-services/package.json` 全部 `name` → `@digua/<svc>`（如 `iam-services` → `@digua/iam-services`），`publishConfig.registry` → `https://registry.npmjs.org/`；`package.json` 根脚本 `packages:build` 的 `--filter` 同步改为 `@digua/*`（未改名会导致 build 脚本 filter 失配，已同步，✓）；`packages/cli/lib/sdk-publish.js` 的 `NPM_PACKAGES` 清单 7 个包全部 `@digua/` scope；`@digua/cli` 为 `private: true` 且不在发布清单（be9aa0f7）。
- **许可证**：根 `LICENSE` 为**新增文件**（Apache-2.0 全文，此前仓库无 LICENSE）；7 个发布包 `license` 由 `UNLICENSED` → `Apache-2.0`；抽查 `libs/common-error/Cargo.toml`、`sdk/rust/audit-service-sdk/Cargo.toml` 均 `license = "Apache-2.0"`（crates.io 发布强制项，✓）。
- 其他关键 diff：`release.toml` `rust_registry` `"kellnr"` → `""`（cargo 默认=crates.io）、`npm_registry` → `https://registry.npmjs.org/`；`pnpm-workspace.yaml` 移除已废弃的 `sdk/config-services`；AGENTS.md 分发模型段落同步改写；`.image.env` 两单元版本 1.7.0→1.8.0（registry 仍为内网 `zhcoder-docker-registry.com:8000`，与 Docker Hub 发布互不影响）。

## 2. workflow_dispatch 能否对已存在 tag 执行 — 能

`.github/workflows/public-release.yml`：

- `workflow_dispatch.inputs.tag`（required，`vMAJOR.MINOR.PATCH`）→ `Resolve target release tag` 步骤正则强校验（非法格式 exit 1）；
- `Verify tag points to main history`：`git fetch origin "+refs/tags/${TAG}:refs/tags/${TAG}"` 显式拉取**已存在**的 tag（不存在即失败），`merge-base --is-ancestor <tag> origin/main` 强制 main 血统（`public-release.yml:57-67`）；
- 全部 job 的 checkout 用 `ref: inputs.tag` 解析既有 tag，**不新建/不移动 tag** ✓。
- 实证：`v1.8.0` tag 本地与远端均存在，指向 `5b675086` = 当前 `origin/main` HEAD，是 main 祖先 → `workflow_dispatch` 输入 `v1.8.0` 即可补发。

## 3. 手动发布是否经过完整验证与 main ancestry 检查 — 是

- 三个 publish job 均 `needs: validate` 且 `if: github.event_name != 'pull_request'`（workflow_dispatch ≠ pull_request → 会跑）；
- `validate` 对 dispatch/tag push 执行完整链条：checkout tag → tag 格式与 ancestry 校验 → `cargo fmt --check` / `clippy --workspace --all-targets -D warnings` / `cargo test --workspace --lib` → 6 个仓库门禁脚本（service-standard / spec / contracts-gen --check / db-naming / pg-migrations / tenant-scope，脚本在该 commit 均存在）→ `dump-openapi-specs.js` + `git diff --exit-code` → CLI 单测 → `packages:build` → 三个 `--dry-run` 发布计划校验。
- CI 实证：head `327b0016` 的 **Validate release candidate = success**；三个 Publish job 正确 **skipped**（PR 事件无凭证）。

## 4. 三个平台的发布参数 — 正确

| 平台 | 凭证 | 命令 | 参数链核对 |
|---|---|---|---|
| crates.io | `CARGO_REGISTRIES_CRATES_IO_TOKEN = secrets.CRATES` | `coder release sdk rust` | `release.toml rust_registry=""` → `cargoPublishArgs` 不加 `--registry` → 走默认 crates.io ✓；依赖拓扑排序 + path 依赖临时改写（`sdk-publish.js:68-90`）✓ |
| npmjs.org | `NODE_AUTH_TOKEN = secrets.NPMPUBLISHKEY` + `setup-node registry-url: https://registry.npmjs.org/` | `coder release sdk npm` | `pnpm publish --access public` + `publishConfig.registry` 双保险 ✓；`@digua/cli` 不在清单 ✓ |
| Docker Hub | `docker/login-action`（`DOCKER_USER_NAME`/`DOCKER_KEY`） | `coder release images --push --registry docker.io --repo "$DOCKER_USER_NAME"` | 版本真相源 = tag checkout 后的根 `Cargo.toml` workspace version = 1.8.0（DRY-RUN 已证实 `release status` 一致）；`test -n "$DOCKER_USER_NAME"` 空值前置拦截；qemu+buildx 多架构；生成的 `.image.env` 以 artifact 上传、不回写 main ✓ |

## 5. 凭证隔离 — 严格

- workflow 顶层 `permissions: contents: read`；
- `validate` job：**零 secrets**；
- 三个 publish job 相互并行、各自只注入己方凭证（crates job 无 npm/docker secret，反之亦然）；docker 的 `DOCKER_USER_NAME` 非机密且仅进该 job 环境；
- PR 事件下所有 publish job 被 `if` 排除 → PR 协作者/CI 日志接触不到任何发布凭证 ✓。

## 6. 重复发布 / 部分失败 / 断点续发 — 机制完备，靠 registry 幂等兜底

- **crates**：`cargo publish` 失败时匹配 `already exists|already uploaded|is already present` → 记入 skipped 继续（`sdk-publish.js:216-221`），半途失败重跑安全；finally 恢复被改写的 Cargo.toml 与 Cargo.lock（`228-233`）。
- **npm**：匹配 `previously published|EPUBLISHCONFLICT|cannot publish over` → 跳过（`sdk-publish.js:305-309`）；private 包跳过。
- **docker**：同名 tag 重推为幂等覆盖（digest 可能变化，属预期）。
- **concurrency**：tag push 与 dispatch 共用 `public-release-tags` 组、不取消（仅 PR 组可取消）→ 重复触发排队串行，重跑时已完成目标逐个跳过/覆盖，不会重复扣发。
- 无 per-target"已发布即短路"的预检查，重跑成本 = 全 job 重过一遍，属可接受的简单设计。

## 7. v1.8.0 能否在不重建 tag 的情况下补发 — 能，前置是 PR #8 先合入 main

- `workflow_dispatch` 的 workflow 文件取自**默认分支** → 必须先把 PR #8 合入 main，dispatch 入口才存在（当前 main 无此 workflow）；
- 合入后手动输入 `v1.8.0`：tag 已存在且 = main HEAD、祖先校验通过、无需任何 tag 操作；
- 预期发布面（与 INVENTORY 一致）：crates 侧 24 个 crate 版本仍为 1.6.0/1.7.0 且 crates.io 已存在（2026-11-21 已公开发布，经 crates.io API 只读抽查 `digua_common_error`/`digua_audit_sdk`/`digua_iam_sdk`/`digua_jwt_ctx` max_version 均为 1.6.0）→ v1.8.0 的 publish-crates **几乎全量跳过、唯一真实新发布是 `digua_http_client@1.7.0`**（crates.io 尚无此名，首次发布不可撤回）；npm 侧 7 个 `@digua/*@0.1.0` 全部首发；docker 侧 8 个镜像 `1.8.0`/`1.8.0-pg` 全部首发。

## 8. 复核中的特别声明（对应任务约束）

- **不因 npm 404 推断 scope 归属**：`@digua/contracts`、`@digua/iam-services` 在 registry.npmjs.org 均返回 404（未发布过）。404 只说明**尚无包**，不说明 `@digua` organization 存在或归当前发布账号所有——org 端点不公开，归属只能由持 token 账号在 npmjs.com 上确认（org 是否存在、账号是否 owner）。**保持"未验证"判定，不采信任何间接推断。**
- **不因 Docker Hub 不可匿名验证推断可发布**：`DOCKER_USER_NAME` 是 secret、不可见；对候选 namespace 的匿名查询 0 结果不代表可发布/不可发布。**保持"未验证"判定。**
- 本复核未执行任何真实发布动作；所有 registry 查询均为匿名只读 GET。

## 9. 问题与建议

| 级别 | 问题 | 建议 |
|---|---|---|
| P2 | `@digua` npm org 归属与 NPMPUBLISHKEY 权限未验证，首次 publish 才会暴露（失败形态 402/403/ENEEDAUTH） | 人工在 npmjs.com 确认 org 存在 + token 账号 owner；或先以最小包试探（会占用公开命名，需 owner 决策） |
| P2 | Docker Hub namespace（DOCKER_USER_NAME）未确认；首推自动建 repo 且公开不可撤回 | 人工核对 secret 指向的 namespace；确认账号邮箱/2FA 与组织策略 |
| P2 | `digua_http_client@1.7.0` 名称未占位，首发不可撤回 | 发布窗口内尽快完成（降低抢注概率），或接受 INVENTORY 已声明的风险 |
| P3 | tag push 直接触发发布，无人工审批门（environment protection rules 未使用） | 建议给 publish jobs 加 GitHub `environment` + required reviewers（单人 repo 可暂缓） |
| P3 | npm 包版本 `0.1.0` / crate 版本 `1.6.0` 与平台版本 `1.8.0` 三线解耦 | INVENTORY 已声明为设计选择；建议在 README 发布说明中向下游明示各制品版本对照 |
| P3 | `.image.env` 在本分支已 bump 1.8.0 但内网 registry 镜像未必已存在 1.8.0 tag | 确认内网 1.8.0 镜像已由既有 GitLab 流水线产出，否则合并后 examples 单元拉取失败（仅影响示例部署单元） |

**结论**：workflow 实现质量达标（隔离正确、门禁完整、幂等/续发机制有效、CI 实证通过），**达到退出 Draft / 可合并的技术条件**；公开发布的放行还差 3 项外部确认（npm org、Docker namespace、法务/owner 批准），见 `RELEASE-GATES.md`。
