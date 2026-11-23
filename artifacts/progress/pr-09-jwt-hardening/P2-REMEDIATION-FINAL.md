# PR #9 P2 修复 — 最终报告（P2-REMEDIATION-FINAL）

- 日期：2026-11-23（Asia/Shanghai）
- 仓库：https://github.com/luqizheng/lmms（PR：https://github.com/luqizheng/lmms/pull/9）
- 分支：`feat/jwt-key-operations-hardening`（本文件所在分支）

## Commit 定位

| 项 | SHA | 说明 |
|---|---|---|
| 代码修复 commit | `0481726f100610b1cefb1ae209cc9ccc1c1f8817` | `fix(token): close keystore lock and backup concurrency gaps`（keystore.rs / keytool.rs / README.md） |
| 本报告所在 commit | 即本文件被提交的 commit（docs commit，紧随其后） | `docs(token): document key backup upgrade procedure and P2 remediation`（runbook + artifacts）——**推送后即为 PR head**，SHA 见下方推送记录与 git log |

> 说明：报告无法在提交前预知自身 commit SHA，故以「本文件所在提交即 docs commit / PR head」定义；
> 修复前旧 head `02721807863bf95f586a4e9e7e718ed076ebdfb7`（及其前的 `39df6ce1`）**均不再是最终 SHA**。

## 一、修复摘要

独立复审（`REVIEW.md`）发现的 4 个 P2 全部完成**代码级/运维流程级**修复，另顺带修复 4 个直接相关的 P3：

| 发现 | 一句话修复 | 性质 |
|---|---|---|
| F1 锁 Drop 无条件删文件 | owner token（UUID v4）+ Drop 前重读核验，不匹配不删 | 代码 |
| F2 stale 回收只看 mtime | 判定模型整体替换为「独占内核锁可获取才接管」，fail-closed | 代码 |
| F3 旧格式备份升级空窗 | runbook §4.1 升级前后强制 checklist + 失败回退 | 运维文档 |
| F4 backup 不持锁/固定 tmp | 全程持锁 + 锁内磁盘重载快照 + 唯一 tmp 名 | 代码 |
| F5 tmp 权限窗口 | unix 0600 直建 | 代码（P3） |
| F6 相对路径 chmod CWD | parent 为 `.` 时跳过 chmod | 代码（P3） |
| F11 restore 读回验证弱 | 升级为全量 checksum 比对 | 代码（P3） |
| F12 total-loss 无回滚目标 | ERROR 日志显性化 + runbook 条目 | 代码+文档（P3） |

## 二、F1–F4 验收矩阵

| # | 原始问题 | 根因 | 修复 | 关键代码位置 | 新增测试 | 结果 | 剩余限制 |
|---|---|---|---|---|---|---|---|
| F1 | Drop 无条件 `remove_file`，暂停恢复的旧持有者删除新持有者的锁 | 锁所有权未绑定 acquire 实例 | UUID v4 owner token 写入锁文件；Drop 重读比对；持有期保持内核锁 | `keystore.rs`: `LockContent`、`KeystoreLock{owner,file}`、`impl Drop`、`read_lock_content[_from]` | 6 个（含 channel 定序并发替换） | ✅ 通过 | Windows 关句柄→删除间极窄窗口（核验收窄）；Unix flock 劝告语义 |
| F2 | mtime>600s 即回收，合法长锁被误删；pid 未用 | 文件年龄≠持有者死亡 | 接管唯一判据=独占内核锁（Unix flock / Windows share_mode(0)）；拿不到→等待至 `LockBusy`；内核释放=持有者死亡（PID 重用免疫）；errno 异常也 fail-closed | `keystore.rs`: `try_lock_kernel_exclusive`、`exclusive_open_options`、`try_acquire`、`acquire_within`；删除 `lock_is_stale`/`LOCK_STALE_AFTER` | 5 个（伪造一天前时间戳不回收、死持有者接管、旧格式接管、竞争恢复、无泄漏） | ✅ 通过 | NFS 等弱一致存储不保证；版本偏斜（新旧二进制并发）不受保护（部署模型整体替换） |
| F3 | PR #7 裸备份升级后不可 restore，运维无强制重备份步骤 | 格式升级不可逆但缺配套流程 | runbook §4.1：升级前核对记录/留存旧备份；升级后强制重做信封备份+checksum 核对+演练验证；验证前不得删旧备份；失败回退路径 | `docs-site/runbooks/jwt-key-operations.md` §4.1、§5、§6、§9 | 不适用（既有 `legacy_bare_keystore_backup_is_rejected` 保持） | ✅ 成文 | 依赖运维执行；无 `--dry-run`（follow-up） |
| F4 | backup 不持锁、快照取自可能陈旧的内存、固定 tmp 名 | backup 未纳入写路径并发协调；快照来源错误 | 全程持同一把锁；锁内 `load`+validate 从磁盘重载（磁盘不可读→明确失败）；锁内快照→checksum→原子写；tmp 唯一名+失败清理；返回 `BackupSummary`（keytool/审计用真实状态） | `keystore.rs`: `write_backup[_inner]`、`BackupSummary`、`disk_keystore_missing`、`atomic_write_bytes`；`keytool.rs` `backup()` | 5 个（陈旧内存、rotate+backup 并发串行前缀断言、backup+restore 并发、同目标并发、失败不损坏） | ✅ 通过 | 锁等待沿用 5s 超时报 `LockBusy` |

## 三、测试命令及真实结果（2026-11-23，本机 Windows host，MSVC 工具链）

```
SWAGGER_UI_DOWNLOAD_URL=file:///…/infrastructure/vendor/swagger-ui-v5.17.14.zip

cargo fmt --all -- --check                                   → 通过
cargo test -p token_service --lib                            → 79 passed / 0 failed（8.6s）
cargo clippy -p token_service --all-targets -- -D warnings   → 0 error
node scripts/check-service-standard.js                       → 通过
node scripts/check-spec.js                                   → 通过
node scripts/contracts-gen.js --check                        → in sync
node scripts/check-db-naming.js                              → 通过
node scripts/dump-openapi-specs.js && git diff --exit-code -- 'services/*/openapi.yaml' → 无漂移
```

结果分级：
- **本次本地测试结果**：上表全部为本机实测（Windows 10，host 原生 MSVC，host 本机 cargo）。
- **历史测试结果**：修复前 head `02721807` 为 66 passed（初版交付 `39df6ce1` 同为 66），仅作对照，非当前结果。
- **GitHub Actions 当前 head 结果**：推送后核对并回填于此（见 §八）；在核对完成前不标注 success。

### 新增测试列表（13 个净增，全部随 `cargo test -p token_service --lib` 执行）

F1：`lock_drop_removes_own_lock`、`lock_drop_does_not_remove_foreign_owner_lock`、
`concurrent_owner_replacement_never_lets_old_owner_delete_new_lock`、
`lock_drop_tolerates_corrupt_or_missing_lock_file`（unix）、
`lock_file_is_exclusive_while_held_on_windows`（windows）
F2：`live_holder_lock_is_never_reclaimed_even_when_aged`、
`dead_holder_lock_is_taken_over_and_ownership_rewritten`、
`legacy_or_corrupt_lock_content_taken_over_only_when_unheld`、
`lock_contention_fails_fast_and_recovers`（重写：真实内核锁持有）、
`sequential_lock_acquires_never_leak_state`
F4：`backup_snapshots_fresh_disk_state_not_stale_memory`、
`concurrent_rotate_and_backup_produce_serialized_consistent_backups`、
`concurrent_backup_and_restore_stay_consistent`、
`concurrent_backups_to_same_target_never_corrupt_each_other`、
`failed_backup_does_not_damage_existing_backup`

并发测试同步方式：`std::sync::Barrier`（rotate/backup、backup/restore、同目标备份）与
`std::sync::mpsc::channel`（owner 替换定序）；**无任何固定 sleep 依赖**（仅锁内部 25ms 退避，
与既有实现一致）；断言均为结构性判定（串行前缀性质、checksum 一致、所有权不匹配不删除），
不依赖偶然调度。

## 四、Windows / Unix 兼容性说明

| 机制 | Unix | Windows | 验证 |
|---|---|---|---|
| 内核互斥 | `flock(LOCK_EX|LOCK_NB)`（extern "C"，无新依赖） | `OpenOptions::share_mode(0)` 独占打开（std） | 各自平台专属测试 |
| 锁文件内容读写 | flock 为劝告锁，路径读写随时可行；测试在持有作用域内断言 payload | 持有期间任何路径打开被 OS 拒绝（含读/写/删除）；测试断言"拒绝"本身 | `lock_drop_tolerates…`（unix）、`lock_file_is_exclusive…`（windows） |
| Drop 顺序 | 读核验→unlink→关句柄（unlink 时 flock 仍持有，无窗口） | 关句柄→读核验→unlink（独占句柄无法 delete-while-open） | 两个平台均跑 `lock_drop_removes_own_lock` |
| 文件权限 | tmp/锁文件 0600 直建（`OpenOptionsExt::mode`），目录 0700 | NTFS ACL，不适用 POSIX mode（与既有实现一致） | `permissive_key_file_fails_closed`（unix，CI Linux） |
| 本次实测平台 | Windows 10 host 全套 79 过 | 同左 | Linux（CI）预期 79（unix/windows 专属测试数相抵） |

## 五、修改文件列表（P2 remediation，fix commit `0481726f` + docs commit）

**fix commit**：
- `services/token-service/src/keystore.rs`（锁重写 / backup 重写 / atomic_write 加固 / restore 校验加强 / 新测试）
- `services/token-service/src/bin/keytool.rs`（backup 输出使用 `BackupSummary`）
- `services/token-service/README.md`（锁机制描述同步）

**docs commit**：
- `docs-site/runbooks/jwt-key-operations.md`（§4.1 升级流程 + §5/§6/§9 同步）
- `artifacts/progress/pr-09-jwt-hardening/FINDINGS.md`（修复状态标注，原文保留）
- `artifacts/progress/pr-09-jwt-hardening/REVIEW.md`（附录：P2 修复复核）
- `artifacts/progress/pr-09-jwt-hardening/P2-FIX-REPORT.md`（新增）
- `artifacts/progress/pr-09-jwt-hardening/REPORT.md`（更新）
- `artifacts/progress/PR-BATCH-REPORT.md`（更新）
- `artifacts/progress/pr-09-jwt-hardening/P2-REMEDIATION-FINAL.md`（本文件，新增）

未纳入：`rust-ci-timings.json`（会话前遗留未跟踪文件，保持未跟踪）。

## 六、尚未解决的风险（保留，未擅自扩大范围）

P3 未修复项（与本次改动无直接耦合，按最小化修改原则留待后续）：
1. **F7** rename 后未 fsync 父目录（断电极端场景 crash consistency）；
2. **F8** 审计写入失败不反馈到 admin API 响应；
3. **F9** 审计 detail 含 keystore SHA-256 摘要（非泄漏，信息面最小化建议）；
4. **F10** 备份信封无 HMAC（威胁模型内非缺陷，文档表述已避免误导）；
5. **F13** `refresh_runtime_state` 失败走 `expect` panic（重启自愈，一次性 5xx）；
6. **F14 部分** 双进程并发首启、ledger 坏行、expect_active_kid 审计断言、total-loss restore 测试仍缺；
7. **F15** ledger 单行损坏使整个账本不可读。

架构性限制（既有，非本次引入）：
- 单实例部署约束不变（多副本不支持，runbook §1）；
- 锁与备份均依赖本地文件系统语义（NFS/多节点共享卷不保证）；
- PR #7 旧格式备份升级依赖运维执行 §4.1 checklist（代码不强制）。

## 七、PR 是否可以进入独立复审

**可以**。F1–F4 已修复并有针对性测试；5 项仓库门禁全过；P3 遗留均不阻断（见 FINDINGS 定级：
无 P0/P1，剩余 P3 为纵深防御/可观测性改进）。建议独立复审聚焦：
1. `KeystoreLock` 内核锁模型（`keystore.rs` try_acquire/Drop）的双平台语义；
2. `write_backup_inner` 的锁内磁盘重载设计（备份语义从"备份内存"变为"备份磁盘"）；
3. runbook §4.1 升级流程的可执行性。

## 八、GitHub Actions 链接及状态

推送后回填：工作流 OpenAPI Generation Validation / Database Dialect Matrix 在本 head 的
run 链接与结论；核对完成前本节不预填 success。

（若 GitHub API 匿名限流导致无法即时查询，以 PR 页面 Checks 标签为准。）

## 九、合规声明

- ✅ 只在 `feat/jwt-key-operations-hardening` 分支工作；未修改 main。
- ✅ 未合并任何 PR；未创建/删除任何 Tag；未触发正式发布；未使用真实发布凭证。
- ✅ `rust-ci-timings.json` 等会话遗留文件未纳入提交；无密钥/token/私钥/备份文件进入 Git。
- ✅ 保持 keystore 文件格式兼容（`KEYSTORE_VERSION=1` 不变），无数据库迁移。
- ✅ 审计文档保留全部原始问题记录，仅追加状态与修复说明。
