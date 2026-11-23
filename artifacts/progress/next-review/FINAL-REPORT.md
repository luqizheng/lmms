# FINAL REPORT — PR #9 审查 / PR #8 复核 / v1.8.0 补发评估

- 日期：2026-11-23
- 输入：PR #9 head `652a9bfa`（= 本地分支）、PR #8 head `327b0016`（已 fetch 核对）、main `5b675086`（= v1.8.0 tag）
- 约束遵守：本次未合并任何 PR、未创建/删除/移动 tag、未执行任何真实发布、未改 Secrets、未改 main、未删任何包/镜像。所有 registry 访问均为匿名只读 GET。
- 明细文档：`../pr-09-jwt-hardening/REVIEW.md` + `FINDINGS.md`；`../v1.8.0-public-release/PR8-REVIEW.md` + `RELEASE-GATES.md`。

---

## 1. PR #9 是否达到合并条件 — **达到（有条件）**

- 0 个 P0/P1；4 个 P2、10 个 P3（全部明细见 `../pr-09-jwt-hardening/FINDINGS.md`）。
- 十个审查维度结论：锁（通过，2 个 P2 并发缺陷）、指纹防陈旧覆盖（通过）、审计覆盖成功/失败且无密钥（通过）、审计失败策略明确可观测（通过）、备份信封拒绝篡改/不兼容版本（通过）、restore 六步安全序列完整（通过）、权限正确（通过）、测试覆盖优秀但缺 7 类异常路径（P3 清单）、与 PR #7 无重复且无冲突、keystore 文件零迁移（KEYSTORE_VERSION 仍为 1，结构未变）。
- 实证：本机 host 工具链 `cargo test -p token_service --lib` → **66 passed / 0 failed**。
- P2 均不阻断合并：F1（锁 Drop 误删他人锁）/F2（stale 判定无 pid 存活校验）触发概率极低且有指纹 CAS 兜底；F3（PR #7 裸备份不可恢复）是刻意 fail-closed 设计，只差 runbook 升级清单里一行"升级后立即重做备份"；F4（并发 backup 无锁）最坏产生被 checksum 拒绝的坏备份，不静默。
- 建议条件：合并时或紧跟一个小 PR 处理 F4（backup 持锁）+ F3 的 runbook 补行；F1/F2 可排入后续。

## 2. PR #8 是否达到退出 Draft 的条件 — **达到（技术上可合并；发布放行另需外部确认）**

（PR #8 当前 state=open 非 draft；"退出 Draft"按可合并标准评估。）

- 当前完整 diff 确认仍含 npm `@digua/*` scope 改名（7 包 + `packages:build` filter 同步）与许可证修改（新增 Apache-2.0 LICENSE + npm 包 UNLICENSED→Apache-2.0）。
- workflow_dispatch 可对已存在 tag 执行：显式 fetch refs/tags + 正则校验 + `merge-base --is-ancestor` main 血统检查，checkout 解析既有 tag，不重建 tag。
- 手动发布经完整 validate（fmt/clippy/test --workspace --lib/6 门禁脚本/openapi drift/CLI tests/build/3 dry-run）+ ancestry 检查；CI 实证 head `327b0016` 的 Validate release candidate = **success**，publish jobs 正确 skipped。
- 发布参数正确（crates.io 默认 registry + `CRATES`；npmjs.org + `NPMPUBLISHKEY` + `--access public`；Docker Hub + `$DOCKER_USER_NAME` repo + buildx 多架构）。
- 凭证严格隔离：validate 零 secrets，三 publish job 各持己方凭证，`permissions: contents: read`。
- 断点续发：cargo `already exists` / npm `previously published` 幂等跳过，docker 重推覆盖，concurrency 排队不取消——部分失败重跑安全。
- 按任务要求的两条否证约束执行：npm 404 仅记为"未发布过"，**不**推断 `@digua` scope 归属；Docker Hub 不可匿名验证仅记为"未验证"，**不**推断可发布。

## 3. v1.8.0 公开补发仍有哪些阻断 — 4 项，全部为人工/流程项

按 `../v1.8.0-public-release/RELEASE-GATES.md`：

1. **PR #8 合入 main**（workflow_dispatch 入口在默认分支，main 现无此 workflow）——本次不执行合并。
2. **npm `@digua` org 归属 + NPMPUBLISHKEY 权限**：org 端点不公开，无法匿名验证，只能持账号人工确认；7 个包均为首发。
3. **Docker Hub 目标 namespace**：`DOCKER_USER_NAME` 不可见；首推自动建公开 repo 且不可撤回，须人工确认 namespace 正确。
4. **公开发布批准记录**（含 `digua_http_client@1.7.0` 名称未占位、首发不可撤回的风险接受）。

技术侧门禁全部就绪：v1.8.0 tag 已存在且 = main HEAD（祖先校验通过），补发**无需任何 tag 操作**；crates.io 上 24 个 digua_* crate（1.6.0/1.7.0，2026-11-21 已发布）会被幂等跳过，实际新增发布面 = `digua_http_client@1.7.0` + npm 7 包（0.1.0）+ Docker Hub 8 镜像（1.8.0 / 1.8.0-pg）。

## 4. PR #3 是否需要优先于 PR #1、#4 — **是**

- **PR #3（MT-03 错误响应脱敏）是安全缺口**：`libs/common-error/src/lib.rs` 的 `From<reqwest::Error>` 把上游错误原文（URL/host/query，可能含 token）经 `IntoResponse` 直接序列化给客户端，缺口真实存在且 0% 落地；修复面小（4 files +110/−2），收益/成本比最高。
- PR #1（脚手架去 Kellnr/去硬编码凭据）重要但属开发体验与方向一致性，且有明确 rebase 前置（crate 版本对齐 crates.io、与 #7/#9 shutdown 对齐）。
- PR #4（CI timings，draft）价值最低：dump 脚本部分已被 #6 取代，只剩 `--timings-file` 一个真实缺口，建议关闭重做为小 PR。
- 顺序：**#3 → #1 → #4 重做**（与 `artifacts/progress/pr-backlog-review/REPORT.md` 的独立结论一致）。

## 5. 下一轮建议执行顺序

1. **合并 PR #9**（本轮结论：达到条件）→ 紧跟小 PR：F4 backup 持锁 + runbook 升级清单补"重做备份"步骤（可含 F1 锁 token 比对、F2 pid 存活校验，改动都很小）。
2. **人工关闭 v1.8.0 的 4 项阻断**：合并 PR #8 → npm org / Docker namespace / 发布批准逐项确认后，`workflow_dispatch` 输入 `v1.8.0` 补发（不重建 tag）。
3. **rebase + 精简 PR #3 并合并**（安全优先）：只取 `digua_common_error` 脱敏 + 回归测试，剥离其附带的 workflow 变更。
4. **rebase PR #1**：crate 版本对齐 crates.io 实际版本（1.6.0/1.7.0），shutdown 逻辑与 #7/#9 对齐后合并。
5. **关闭 PR #2 / #5**（标记 superseded），**PR #4 关闭重做**为只含 `--timings-file` 的小 PR。
6. （可选）PR #9 的 P3 清单按 F15（ledger 坏行容忍）→ F14（测试缺口）顺序排入后续迭代。

完成后停止，等待人工审查。
