# v1.8.0 公开补发 — 发布门禁复核（RELEASE GATES）

- 复核时间：2026-11-23 ｜ head：PR #8 `327b0016` ｜ 目标 tag：`v1.8.0`（已存在）
- 性质：只读复核。未执行任何发布、未动 tag / secrets / main / registry。

## 门禁总表

| # | 门禁 | 状态 | 证据 |
|---|---|---|---|
| G1 | 发布 workflow 已存在于 main | ⛔ 阻断 | workflow_dispatch 入口取自默认分支；PR #8 尚未合并，main 无 `public-release.yml` |
| G2 | v1.8.0 tag 可补发（不重建） | ✅ 通过 | tag 存在（本地+远端），指向 `5b675086` = origin/main HEAD，`merge-base --is-ancestor` 通过；dispatch 输入 `v1.8.0` 即可，workflow 内置格式正则 + ancestry 校验 |
| G3 | validate 门禁全绿 | ✅ 通过（CI 实证） | head `327b0016` 的 **Validate release candidate = success**（fmt/clippy/test --workspace --lib/6 脚本门禁/openapi drift/CLI tests/packages build/3 dry-run）；publish jobs 正确 skipped |
| G4 | crates.io 凭证可用 | ⚠️ 未验证 | `secrets.CRATES` 存在性/有效性不可离线验证；失效时 publish-crates 失败，重跑不产生坏状态 |
| G5 | crates.io 内容正确性 | ✅ 通过（含一项风险知悉） | 24 个 digua_* crate 已在 crates.io（1.6.0/1.7.0，2026-11-21 发布，API 只读抽查确认）→ v1.8.0 重跑全部 already-exists 跳过；**唯一真实首发 `digua_http_client@1.7.0`**：名称未占位、首发不可撤回、有抢注窗口（P2，需 owner 知悉） |
| G6 | npm scope 归属与凭证 | ⛔ 阻断（未验证） | `@digua/*` 7 包 registry 均返回 404 = 未发布过；**404 不构成归属或可发布性的证据**。`@digua` org 是否存在、`NPMPUBLISHKEY` 账号是否 org owner，只能人工在 npmjs.com 确认。未确认前首次 publish 大概率 403/ENEEDAUTH |
| G7 | npm 包合规 | ✅ 通过 | license 全部 Apache-2.0（不再 UNLICENSED，npmjs 拒绝 UNLICENSED 公开发布）；`@digua/cli` 已移出发布面；`--access public` 正确（scope 包默认 private） |
| G8 | Docker Hub namespace 与凭证 | ⛔ 阻断（未验证） | `DOCKER_USER_NAME`/`DOCKER_KEY` 为 secret 不可见；候选 namespace 匿名查询 0 结果**不构成任何归属结论**。首推自动建公开 repo、不可撤回，须人工先确认 namespace 目标正确 |
| G9 | Docker 发布参数 | ✅ 通过 | `--registry docker.io --repo "$DOCKER_USER_NAME"` + 空值拦截；版本 1.8.0/1.8.0-pg 来自 tag checkout 的 workspace version（dry-run 已证一致）；多架构 buildx；`.image.env` 产物走 artifact 不回写 main |
| G10 | 凭证隔离 | ✅ 通过 | validate 零 secrets；三 publish job 各持己方凭证；`permissions: contents: read`；PR 事件全部 publish 被排除 |
| G11 | 幂等 / 断点续发 | ✅ 通过 | cargo publish `already exists` 跳过；pnpm publish `previously published` 跳过；docker 重推幂等覆盖；concurrency 组排队不取消 |
| G12 | 许可证/法务批准 | ⚠️ 缺记录 | LICENSE（Apache-2.0）与各包 license 声明齐备；但"公开发布"的 owner/法务批准无书面记录，需项目负责人确认（INVENTORY 已列） |
| G13 | 版本基线一致 | ✅ 通过 | workspace 1.8.0 = 8 服务继承 = 两单元 `.image.env` 钉版（DRY-RUN `release status` 一致）；npm 0.1.0 / crates 1.6.0-1.7.0 与平台版本解耦属已声明的设计选择 |

## 阻断清单（放行前必须逐项关闭）

1. **G1**：PR #8 合入 main（本次审查不执行合并）。
2. **G6**：人工确认 `@digua` npm organization 归属 + `NPMPUBLISHKEY` 账号具备 org 发布权限。
3. **G8**：人工确认 `DOCKER_USER_NAME` 指向的目标 namespace（账号归属、repo 可见性策略）。
4. **G12**：项目负责人对公开发布的正式批准（含 `digua_http_client` 抢注风险的接受，G5）。

## 已知非阻断事项

- crates.io 侧对 v1.8.0 几乎是空操作（幂等跳过），实际新发布物 = `digua_http_client@1.7.0` + npm 7 包 + docker 8 镜像。
- npm/crates 版本与平台版本三线解耦：下游文档需给出对照表。
- 发布部分失败后的补发路径已验证机制层面安全（逐目标跳过/覆盖），无需"断点续发"专门工具。
- 本分支 `.image.env` 已指向内网 registry 的 1.8.0 tag：需确认 GitLab 既有流水线已产出这些内网镜像，否则合并后示例部署单元会拉取失败（仅影响 examples，不影响公开发布本身）。

## 放行判定

**当前不具备正式补发条件。** 技术侧门禁（G2/G3/G5 内容/G9/G10/G11/G13）全部通过；阻断全部集中在外部人工确认项（G1 合并、G6 npm org、G8 Docker namespace、G12 批准）。四项关闭后，通过 `workflow_dispatch` 输入 `v1.8.0` 一次触发即可完成补发，无需任何 tag 操作。
