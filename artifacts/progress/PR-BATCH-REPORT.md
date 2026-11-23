# 连续 PR 开发任务批次 — 总报告（PR #8 + PR #9）

- 仓库：https://github.com/luqizheng/lmms.git
- 生成时间：2026-11-23（Asia/Shanghai）
- 本文件位于 PR #9 分支 `feat/jwt-key-operations-hardening`

## 一、总体结论

| PR | 标题 | 分支 | 地址 | 最终 SHA | 状态 |
| --- | --- | --- | --- | --- | --- |
| #8 | feat(release): add tag-triggered public publishing pipeline | `feat/public-release-pipeline` | https://github.com/luqizheng/lmms/pull/8 | `327b0016cf38ea665e0949d209b4a89834bbac2a`（快照） | open，未合并 |
| #9 | feat(token): JWT key lifecycle operations hardening | `feat/jwt-key-operations-hardening` | https://github.com/luqizheng/lmms/pull/9 | `39df6ce1ea6ace8a2d9984d904cc312087c6c4a6` | open，未合并 |

> 注：PR #8 分支在本报告编写期间仍被并发迭代（head SHA 持续前进），上表 SHA 为快照；以 PR 页面最新值为准。PR #8 自带收尾报告位于其分支 `artifacts/progress/pr-08-public-release/REPORT.md`。

## 二、PR #8 完成情况（公开发布流水线）

- 能力：Tag（`v*`）触发发布；验证与发布分离；Rust（crates.io）/ npm（npmjs.org，`--access public`）/ Docker（版本化，不覆盖 latest）独立发布 Job；发布 Job 依赖完整验证；凭证仅在对应步骤使用；排除内部 `@digua/cli`；Cargo 默认 registry 为 crates.io；含发布说明文档与回归测试。
- 已修复问题（PR #8 报告）：Cargo 误传 `--registry null`；contracts 缺少 TypeScript 依赖致 `pnpm install` 失败；`packages:build` 引用已移除的 `config-services`；main 的 OpenAPI 1.7.0/1.8.0 版本漂移；`.image.env` 版本滞后；`check-tenant-scope` 误报（以 `delegated` 豁免注释核实下游 SQL）。
- CI（快照 head `327b0016`）：Public Release success，OpenAPI Generation Validation success，Database Dialect Matrix in-progress（更早的 head 上已 success）。PR #8 处于活跃收尾中。
- 未创建 `v*` Tag、未真实发布、未修改 Secrets、未合并。

## 三、PR #9 完成情况（JWT 密钥运维加固）

- 实证发现 5 类风险（审计缺失、备份无完整性/版本、恢复无快照/读回/回滚、并发丢失更新、多实例不可用），逐项最小化加固。
- 关键改动：持久审计账本 `keystore-audit.jsonl`（结构对齐 `audit_events`，绝不含密钥原文）；跨进程锁 + 写前 SHA-256 指纹（陈旧写 `Stale`，不静默覆盖）；备份版本化信封 + checksum + 恢复前快照 + 读回校验 + 自动回滚；单实例部署不引入分布式锁并记录扩展条件；新增运维手册。
- 测试：`cargo test -p token_service --lib` → 66 passed / 0 failed；Clippy 无告警；5 项门禁通过。
- CI（`39df6ce1`）：OpenAPI Generation Validation [45130581838] success；Database Dialect Matrix [45130581924] success。
- 详细报告：`artifacts/progress/pr-09-jwt-hardening/REPORT.md`；调查：`INVESTIGATION.md`。

## 四、CI 总体状态

| 分支 | 工作流 | 结果 |
| --- | --- | --- |
| PR #9 `feat/jwt-key-operations-hardening` | OpenAPI Generation Validation | success（45130581838） |
| PR #9 `feat/jwt-key-operations-hardening` | Database Dialect Matrix | success（45130581924） |
| PR #8 `feat/public-release-pipeline`（快照 `327b0016`） | Public Release | success |
| PR #8 `feat/public-release-pipeline`（快照 `327b0016`） | OpenAPI Generation Validation | success |
| PR #8 `feat/public-release-pipeline`（快照 `327b0016`） | Database Dialect Matrix | in-progress（更早 head 已 success） |

本仓库 GitHub Actions 在 main 上仅有两个工作流（`openapi-generation-validation.yml`、`dialect-matrix.yml`）；PR #8 分支额外新增 `public-release.yml`，同一 PR 上以验证模式运行。

## 五、主要技术改动

- **PR #8**：`.github/workflows/public-release.yml`（Tag 触发、校验 SemVer、Tag 必须在 main 历史、三类发布独立 Job、凭证作用域最小化、Docker 版本 Tag 不可变）；`packages/contracts` 中文错误码补齐；`.image.env` 钉版同步。
- **PR #9**：`token-service` 新增 `audit.rs`，改造 `keystore.rs`（锁/指纹/信封/快照/回滚）、`keys_admin.rs`（在线审计上下文）、`keytool.rs`（`audit` 子命令）；新增运维手册与 spec。

## 六、安全性评估

- 二者均未合并、未打 Tag、未真实发布、未改 Secrets、未删除密钥、未执行不可逆 DB 操作、未修改 main。
- PR #8：发布凭证仅在对应 Job 步骤注入；对外仅发布版本化 Docker 镜像，不覆盖 latest；排除内部包。
- PR #9：审计账本与日志不含 PEM/私钥/凭证；恢复具备完整性校验与失败回滚，避免密钥不可恢复；并发写显式失败而非静默覆盖。

## 七、剩余问题与风险

1. PR #8 分支仍在活跃迭代，最终 SHA 以其收尾为准；main 在 PR #8 合入前仍可能为红（源自 main 的版本升级提交未配套更新 spec/钉版）。
2. PR #9 审计账本暂未投递 audit-service（离线 keytool 无 DB/MQ）；多实例水平扩展不支持（当前单实例交付）。
3. `scripts/update-image-env.js` 的 `BASE_IMAGES` 未含 `IMAGE_NGINX`（PR #8 已记录，超出范围未改）。

## 八、需人工决策项

1. 是否将 token-service 密钥审计接入 audit-service 事件管道。
2. 若未来多实例部署，共享密钥存储与一致性方案选型。
3. PR #8 / PR #9 的评审与合并顺序（建议先合 PR #8 修复 main 红灯，再合 PR #9）。

## 九、是否达到合并条件

- 两个 PR 在各自最终提交上 CI 通过、无阻断性遗留问题，**达到可提交人工评审/合并的条件**（以 PR 页面最新 CI 为准）。
- 本批次**不自动合并**，等待项目负责人统一审查。

## 十、明确声明

- ✅ **未执行任何正式发布**：未上传 crates.io / npmjs.org / Docker Hub。
- ✅ **未创建任何正式版本 Tag**。
- ✅ **未合并任何 PR**（PR #8、PR #9 均为 open）。
- ✅ **未修改 GitHub Secrets**，未使用真实发布凭证。
- ✅ **未删除任何现有密钥**，未执行不可逆数据库操作，未修改 main。
