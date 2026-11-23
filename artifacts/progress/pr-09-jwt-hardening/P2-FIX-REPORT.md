# PR #9 P2 修复报告（F1–F4）

- 修复 commit：`0481726f100610b1cefb1ae209cc9ccc1c1f8817`
  （`fix(token): close keystore lock and backup concurrency gaps`）
- 原始发现：[`FINDINGS.md`](./FINDINGS.md)（原文保留）；复审结论：[`REVIEW.md`](./REVIEW.md) 附录
- 本报告对应上述 fix commit 的代码状态；测试为 2026-11-23 本机 Windows host 实测

---

## F1 — 锁所有权与 Drop 误删

| 项 | 内容 |
|---|---|
| 原始问题 | `KeystoreLock::drop()` 无条件 `remove_file`：持有者被暂停 → 锁被 stale 回收 → 新持有者建锁 → 旧持有者恢复后 Drop 删除**新持有者的锁**，第三进程可并发获锁 |
| 根因 | 锁文件只写了 `pid= at=` 且 Drop 不做所有权核验；锁的"所有权"没有绑定到具体一次 acquire |
| 修复方案 | ① 每次 acquire 生成 UUID v4 owner token（`uuid::Uuid::new_v4()`，getrandom 来源，非 PID）；② 锁文件 payload 为 JSON `LockContent{format_version, owner, pid, created_at}`；③ `KeystoreLock` 实例持有自己的 token；④ Drop 删除前重读锁文件并比对 owner——不匹配/损坏/缺失一律不删、不 panic |
| TOCTOU 处理 | 持有期间进程保持锁文件句柄打开并持有**独占内核锁**（见 F2）。Unix：flock 为劝告锁，"读→删"在 flock 仍持有期间完成，协作竞争者无法在窗口内接管；非协作写者可改 payload，但改写后 owner 不匹配 → Drop 不会删（这正是防御目标）。Windows：句柄以 `share_mode(0)` 打开，持有期间任何外部打开/改写/删除都被 OS 拒绝；Drop 顺序为「先关句柄 → 重读核验 → 删除」，关-删之间由 owner 核验二次收窄 |
| 关键代码 | `services/token-service/src/keystore.rs`：`struct LockContent`、`struct KeystoreLock{path, owner, file}`、`impl Drop for KeystoreLock`、`read_lock_content()`、`read_lock_content_from()` |
| 新增测试 | `lock_drop_removes_own_lock`、`lock_drop_does_not_remove_foreign_owner_lock`（unix 改写场景 / windows 独占拒绝场景）、`concurrent_owner_replacement_never_lets_old_owner_delete_new_lock`（channel 定序）、`lock_drop_tolerates_corrupt_or_missing_lock_file`（unix）、`lock_file_is_exclusive_while_held_on_windows`（windows）、`lock_drop_removes_own_lock` 断言 payload 形状 |
| 测试结果 | 全部通过（本机 Windows `cargo test -p token_service --lib` 79 passed / 0 failed；unix 分支测试由 CI Linux 执行） |
| 剩余限制 | Windows 关句柄→核验→删除之间存在极窄理论窗口（owner 核验收窄 + 残留锁由接管路径自愈）；Unix flock 对非协作进程是劝告语义 |

## F2 — stale 锁回收策略

| 项 | 内容 |
|---|---|
| 原始问题 | `lock_is_stale()` 仅看 mtime>600s 即删锁：合法长运行/挂起进程的锁被误回收（与 F1 复合放大）；锁文件里的 pid 从未被使用 |
| 根因 | 用"文件年龄"近似"持有者死亡"，这两者没有必然关系；且未定义 pid 检查，PID 重用问题无从谈起 |
| 修复方案 | **整体替换判定模型**：回收/接管唯一判据 = **独占内核锁能否获取**。持有者存活 → 内核锁被占 → 任何接管尝试失败 → 候选者退避等待至 `LOCK_WAIT`(5s) 超时返回 `LockBusy`（fail-closed）；持有者退出/崩溃 → 内核自动释放（Unix flock 随进程死亡释放；Windows 句柄随进程死亡关闭）→ 接管成功并重写 payload。**liveness 由内核裁决**：不存在"PID 存活但锁是残留"的歧义，天然免疫 PID 重用；也不再有"无法检查 PID 就视为 stale"的危险路径 |
| fail-closed 细节 | Unix `flock` 返回 EAGAIN/EINTR → 忙等；其他 errno（EINVAL/EOPNOTSUPP/ENOLCK 等，奇异文件系统）→ 视为"无法判定"→ 同样等待至 `LockBusy`，绝不接管。Windows 打开失败（共享冲突/权限/其他）→ 一律 Busy |
| 平台差异 | Unix：`flock(LOCK_EX|LOCK_NB)`（extern "C" 声明，不新增依赖）；Windows：`OpenOptions::share_mode(0)`（std 自带）。fresh 创建路径（`create_new`）在 Unix 上也立即 flock，关闭"创建成功但尚未持锁"的竞态窗口 |
| 可观测日志 | 接管时记录原因（kernel lock released）、前持有者 pid/created_at/锁龄（payload 可解析时）；无密钥内容 |
| 关键代码 | `keystore.rs`：`try_lock_kernel_exclusive()`、`exclusive_open_options()`、`KeystoreLock::try_acquire()`、`acquire_within()`；已删除 `lock_is_stale()` 与 `LOCK_STALE_AFTER` |
| 新增测试 | `live_holder_lock_is_never_reclaimed_even_when_aged`（伪造一天前时间戳 + 持有内核锁 → `LockBusy`）、`dead_holder_lock_is_taken_over_and_ownership_rewritten`（死持有者接管 + ownership 重写）、`legacy_or_corrupt_lock_content_taken_over_only_when_unheld`（旧格式/损坏 payload 且无内核锁 → 安全接管重写）、`lock_contention_fails_fast_and_recovers`（重写为真实内核锁持有）、`sequential_lock_acquires_never_leak_state` |
| 测试结果 | 全部通过（同上）；`LockBusy` 消息含 "live" 策略字样，有断言 |
| 剩余限制 | flock/独占打开依赖本地文件系统语义，NFS/多节点共享卷不保证（runbook §9 已声明）；版本偏斜场景（旧版二进制与新版并发运行）下旧版的 create/remove 行为不受新锁保护——部署模型为整体替换镜像，不构成实际暴露 |

## F3 — 旧格式备份升级运维流程

| 项 | 内容 |
|---|---|
| 原始问题 | PR #7 生成的裸 keystore 备份（无信封/无 checksum）在 PR #9 restore 中被 `BackupVersion` 拒绝；若运维不在升级当天重做备份，灾难恢复存在空窗；runbook 未把"升级后重做备份"列为必做项 |
| 根因 | 格式升级不可逆（旧格式无完整性校验，静默兼容会绕过 fail-closed 校验），但缺少配套的强制运维流程 |
| 修复方案 | 纯运维方案（未降低备份验证安全性）：runbook 新增 §4.1「PR #7 旧格式备份升级」——升级前检查（旧版本健康、记录 active kid/密钥数量/状态/时间戳、旧备份安全留存）；升级后强制步骤（确认 inspect 与记录一致 → 新 keytool 重做信封备份 → 核对 `keystore_sha256` → 演练环境验证可 restore → 存密管登记 checksum），并明确"完成验证前不得删除旧备份"；升级失败回退说明（回旧镜像 + 旧版本恢复）；§5/§6/§9 同步更新 |
| 关键代码 | `docs-site/runbooks/jwt-key-operations.md` §4.1、§5 第 1 步、§6 故障表、§9 风险 |
| 新增测试 | 无（文档项）；代码拒绝路径既有测试 `legacy_bare_keystore_backup_is_rejected` 保持有效 |
| 测试结果 | 不适用 |
| 剩余限制 | 依赖运维执行 checklist；代码不提供 `--dry-run`（spec follow-up） |

## F4 — 并发 Backup 一致性

| 项 | 内容 |
|---|---|
| 原始问题 | `write_backup_inner` 不持锁、直接从内存 `self.data` 构造备份；固定 `.tmp` 文件名 → 并发备份交错写坏文件；内存快照可能陈旧 |
| 根因 | backup 未纳入 keystore 写路径的并发协调机制；快照来源是加载时刻的内存而非锁保护时刻的磁盘 |
| 修复方案 | ① `write_backup_inner` 全程持**同一把**跨进程锁（与 rotate/restore/save 一致）；② 锁内先 `Self::load` 从磁盘**重载并全量校验**，备份磁盘真实状态——内存陈旧不再进入备份；磁盘不存在（first-boot 窗口）才回退内存快照（WARN 日志）；磁盘存在但不可读/损坏 → 备份**明确失败**，绝不静默备份旧内存（fail-closed）；③ 顺序 = 取锁 → 重载快照 → 构造 envelope → 计算 checksum → 原子写，全部锁内完成；④ tmp 文件名唯一（`<name>.tmp.<uuid v4>`），失败清理，不残留；⑤ 无同进程重复获锁路径（`load`/`persist_locked`/`atomic_write` 均不再取锁）→ 无死锁；⑥ 失败只影响本次目标文件，既有备份不受损（tmp+rename 原子性）；⑦ `BackupEnvelope` 外部格式未变 |
| 陈旧快照评估 | `KeyStore` 内存快照在多进程下**可能**陈旧（设计即单实例+显式 Stale 错误）。修复后 backup 不依赖内存新鲜度：以锁内磁盘重载为准，`write_backup` 返回 `BackupSummary{active_kid, keys, keystore_sha256}`（keytool 输出与审计 detail 使用重载后的真实状态，不再是内存值） |
| 关键代码 | `keystore.rs`：`write_backup()`（返回 `BackupSummary`）、`write_backup_inner()`、`disk_keystore_missing()`、`atomic_write_bytes()`（唯一 tmp + 失败清理）；`bin/keytool.rs` `backup()` 打印 summary |
| 新增测试 | `backup_snapshots_fresh_disk_state_not_stale_memory`（A 陈旧内存 + B 已 rotate → 备份含 B 的状态且 checksum 与磁盘一致）、`concurrent_rotate_and_backup_produce_serialized_consistent_backups`（3 rotate + 3 backup barrier 并发 → 每个备份内部 checksum 一致且 keys 为最终磁盘状态的**串行前缀**，撕裂/陈旧快照必违反前缀性质）、`concurrent_backup_and_restore_stay_consistent`（backup 与 restore barrier 并发 → 备份为合法串行状态、终态确定）、`concurrent_backups_to_same_target_never_corrupt_each_other`（4 并发同目标 → 文件完整 + 无 tmp 残留）、`failed_backup_does_not_damage_existing_backup`（失败不损坏既有备份 + tmp 清理） |
| 测试结果 | 全部通过（同上）；并发测试用 barrier/channel 定序，无固定 sleep |
| 剩余限制 | backup 的锁等待沿用 `LOCK_WAIT=5s`，被长操作阻塞时报 `LockBusy`（预期行为） |

---

## 测试与门禁（对应 fix commit `0481726f`）

| 检查 | 命令 | 结果（2026-11-23 本机 Windows） |
|---|---|---|
| 格式 | `cargo fmt --all -- --check` | 通过 |
| 单元测试 | `cargo test -p token_service --lib` | **79 passed / 0 failed**（66 既有 + 13 新增；Linux CI 预期同为 79：unix/windows 专属测试数相互抵消） |
| Clippy | `cargo clippy -p token_service --all-targets -- -D warnings` | 0 error（仅既有 workspace unused-dep manifest 警告） |
| 服务规范 | `node scripts/check-service-standard.js` | 通过 |
| 契约规范 | `node scripts/check-spec.js` | 通过 |
| 契约漂移 | `node scripts/contracts-gen.js --check` | in sync |
| 表名前缀 | `node scripts/check-db-naming.js` | 通过 |
| OpenAPI 漂移 | `node scripts/dump-openapi-specs.js` + `git diff` | 无漂移（未改路由） |

历史测试结果（修复前 head `02721807`：66 passed）与本次结果（79 passed）已区分；GitHub Actions
当前 head 结果见 `P2-REMEDIATION-FINAL.md`（推送后核对，未完成前不标注 success）。
