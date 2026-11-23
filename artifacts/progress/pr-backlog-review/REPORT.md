# 历史 PR 积压审查报告

- 审查时间：2026-11-23（Asia/Shanghai）
- 基线：`main` @ `5b675086`（= `v1.8.0` tag）
- 方法：GitHub REST API 读取 PR 元数据/文件清单 + 在 main 上逐项检索代码证据（`git grep` / 读源码），不以 PR 状态代替实际完成度
- 范围：PR #1–#7

## 0. 总览

| PR | 标题 | 状态 | 变更规模 | 结论 |
|---|---|---|---|---|
| #1 | MT-01: harden Rust service scaffold | open | 4 files +202/-138 | **仍必要**，rebase 后保留 |
| #2 | MT-02: repair dialect matrix | closed（未合并） | 13 files +129/-845 | **核心已落地**，保持关闭 |
| #3 | MT-03: sanitize error responses | open | 4 files +110/-2 | **仍必要（安全）**，优先合并 |
| #4 | chore(ci): timings JSON | open（draft） | 3 files +103/-15 | **部分仍必要**，重做为小 PR |
| #5 | fix(ci): restore Rust CI | closed（未合并） | 21 files +364/-37 | **大部分已落地**，保持关闭 |
| #6 | perf(openapi): batch generators | merged | 9 files +138/-29 | 已生效 |
| #7 | feat(token): JWT key persistence | merged | 30 files +2340/-144 | 已生效 |

---

## PR #1 — MT-01 服务脚手架加固（open）

**原始目标**：移除技能/`.cargo/config.toml` 中的私有 Kellnr registry；移除硬编码开发 DB 凭据；MySQL 脚手架示例补齐连接池、DB 就绪探针、Prometheus 文本暴露、不泄露的公开错误；Docker 示例去掉私有 registry 凭据；修正脚手架依赖清单（补 `thiserror`、`digua_common_consul`）；附带 iam-service 的 Swagger opt-in 与优雅停机。

**main 现状（证据）**

- `.cargo/config.toml`：仍保留 `[source.kellnr]` 私有源声明。
- `.agents/skills/coder-new-service/SKILL.md`：仍要求 `registry = "kellnr"`，并宣称「libs/* 发布的 17 个 crate」从私有源拉取。
- `services/iam-service/src/main.rs:125`：仍 `std::process::exit(0)` 关闭（SIGTERM 不进入优雅停机）。
- `services/iam-service/src/main.rs:97-99`：`SWAGGER_ENABLED` 默认 `true`（PR 想改为默认关闭/显式 opt-in）。

**已完成 / 未完成**：4 个文件均未进入 main，功能 0% 落地。

**重复实现**：无。与 PR #5 的 Clippy 修复无交集。

**保留价值**：高。现状会让下游按技能文档接入一个**已不存在的私有 registry**，直接编译失败；这与 v1.8.0 的「公开 crates.io 发布」方向正面冲突。

**建议**：rebase 后继续保留。注意两点：① 技能中的 crate 版本应改成 crates.io 实际版本（1.6.0/1.7.0），而非 `0.1`；② iam 优雅停机改动需与 #7/#9 的 shutdown 逻辑对齐，避免冲突。

---

## PR #2 — MT-02 方言矩阵修复（closed，未合并）

**原始目标**：修复迁移 fixture 与服务测试矩阵；彻底移除 Dameng / MSSQL；只保留 MySQL / PostgreSQL / SQLite。

**main 现状（证据）**

- `libs/common-db/src/dialect/mod.rs`：`DialectKind` 只有 `Mysql / Sqlite / Postgres`，注释明确「Dameng 与 MSSQL 已移除」。
- `libs/common-db/src/dialect/`：不存在 `dameng.rs` / `mssql.rs`。
- `.github/workflows/dialect-matrix.yml:7`：已声明「仅支持 MySQL / PostgreSQL / SQLite；不支持 Dameng / MSSQL」。
- 全仓 `dameng|mssql` 命中仅剩历史计划文档（`docs/superpowers/plans/2026-06-10-*` 等）。

**已完成 / 未完成**：方言移除**已在 main 落地**（等价实现，来源不是本 PR）。PR 分支大量文件是删除 main 上已不存在的文件，rebase 必冲突且无收益。
**未能确认**：stale PG fixture（config-service 相关）与「按 manifest 选择 feature 的服务测试调用」是否另有修复——main 上仍有 `config-service` 的历史注释引用，但无实际模块。

**重复实现**：核心目标已被 main 的等价提交覆盖。

**保留价值**：低。

**建议**：保持关闭（标记 superseded）。若 CI dialect matrix 仍因 fixture 失败，另开一个只含 fixture 修复的小 PR，不要复活本分支。

---

## PR #3 — MT-03 错误响应脱敏（open）

**原始目标**：`digua_common_error` 不再把 Internal / Upstream 明细序列化给 HTTP 客户端；明细仅保留在 Display/source 供日志；客户端可操作错误保持原文；加回归测试。

**main 现状（证据）**

- `libs/common-error/src/lib.rs:90-93`：`From<reqwest::Error>` 直接 `AppError::external("HTTP_ERR", e.to_string())`。
- 同文件 `103-121`：`IntoResponse` 把 `self.message` 原样写入 `ErrBody`。
- → 上游 URL、内部主机名、查询串（可能含 token）会**原样返回给客户端**。
- `public_message()` 脱敏函数不存在；无对应测试。

**已完成 / 未完成**：0% 落地，缺口真实存在。
**重复实现**：无（PR #5 不涉及）。PR 附带的新 workflow 与现有 CI 有重叠，可只取代码+单测。

**保留价值**：高（安全）。
**建议**：rebase 后优先合并；建议触发点时把 `sqlx` 的 Internal 也纳入（当前 SQL 路径 message 已是通用文案，风险低，但 `AppError::internal` 仍可能被调用方传入明细，测试应覆盖）。

---

## PR #4 — 本地 CI 耗时统计导出（open，draft）

**原始目标**：给 `scripts/ci-local-run.js` 加 `--timings-file=<path>`；恢复 `scripts/dump-openapi-specs.js`；修正 CI 注释。

**main 现状（证据）**

- `scripts/ci-local-run.js`：**无** `timings` 相关实现（全文件检索为空）。
- `scripts/dump-openapi-specs.js`：**已存在**，且是 #6 的批量构建增强版（优于本 PR 的顺序 `cargo run`）。

**已完成 / 未完成**：OpenAPI 脚本部分已被 #6 取代；耗时统计仍是真实缺口。
**重复实现**：脚本恢复部分与 #6 重复。
**保留价值**：中（仅 timings 部分）。
**建议**：关闭当前 draft，另开只含 `--timings-file` 的小 PR（不要带回旧版 dump 脚本，避免回退 #6 的性能修复）。

---

## PR #5 — Rust CI 主分支阻塞修复（closed，未合并）

**原始目标**：恢复 OpenAPI gate；把 phase 级 JSON timings 带入主 runner；忽略 pnpm 透传的 `--`；把 `Result<(), ()>` 改为类型化 `RateLimitError`；修 Clippy；补 Rust CI workflow。

**main 现状（证据）**

| 子项 | main 状态 |
|---|---|
| `scripts/dump-openapi-specs.js` | 已存在（#6 批量版） |
| 类型化限流错误 | **已实现**：`libs/common-ratelimit/src/lib.rs:93` 有 `pub struct RateLimited`，`check()` 返回 `Result<(), RateLimited>`（命名不同，能力等价） |
| 忽略 `--` | **已实现**：`ci-local-run.js:372` `if (a === '--') continue;` |
| phase 级 timings JSON | **缺失** |
| 独立 Rust CI workflow | **缺失**（`.github/workflows/` 仅 dialect-matrix / openapi-generation-validation / public-release） |
| Clippy 修复 | 无法精确确认，需按当前 CI 结果判断 |

**已完成 / 未完成**：4/6 子项已由 main 或其它途径覆盖；timings 与独立 Rust CI workflow 仍缺。
**重复实现**：限流类型化、OpenAPI 脚本、`--` 处理均重复。
**保留价值**：低（21 文件、多关注点、大部分已过时）。
**建议**：保持关闭。若仍需 Rust CI workflow / timings，单独立项。

---

## 专项结论（按要求逐条）

1. **PR #2 的 Dameng / MSSQL 支持是否仍然缺失**：不缺失。代码层已彻底移除，`DialectKind` 仅三种，CI 与文档一致；属预期结果，不是回归。
2. **PR #5 的 Rust CI 修复是否仍然缺失**：部分缺失。类型化错误、`--` 处理、OpenAPI 脚本已在 main；**独立 Rust CI workflow 与计时导出仍缺失**。
3. **PR #3 的错误响应是否可能泄露内部信息**：**是**。`reqwest::Error` 的 `to_string()` 直接作为对外 `message` 序列化，可泄露上游 URL/host/query。
4. **PR #1 的服务模板改动是否仍然必要**：必要。技能与 `.cargo/config.toml` 仍指向已废弃的私有 Kellnr registry。
5. **PR #4 的本地 CI 耗时统计是否已被替代**：**没有**。`ci-local-run.js` 至今无 timings 实现。

## 建议处置顺序

1. PR #3（安全，优先）→ rebase + 精简 workflow。
2. PR #1（对外可用性）→ rebase + 版本号对齐 crates.io。
3. 关闭 PR #2、PR #4、PR #5（或转为 issue）。
4. timings / Rust CI workflow 作为独立小任务重新发起。
