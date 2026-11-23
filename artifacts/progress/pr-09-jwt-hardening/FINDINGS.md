# PR #9 FINDINGS — 问题清单（P2/P3）

按优先级排列。P0/P1：无。每个问题含位置、影响、修复建议。

> **修复状态（2026-11-23 P2 remediation）**：F1–F4 已在本轮修复（见 `P2-FIX-REPORT.md`
> 与 `P2-REMEDIATION-FINAL.md`），顺带修复 F5/F6/F11/F12 四个直接相关的 P3。
> 以下为原始发现记录，**原文保留未删改**；每条标题行后追加 `[已修复@<commit>]` 或
> `[未修复，后续跟进]` 状态标注。修复 commit 以 PR 最新 head 为准（fix commit 与
> docs commit 见 P2-REMEDIATION-FINAL.md 首部）。

---

## P2

### F1 [P2][已修复] 锁 Drop 无条件删文件：stale 回收后原持有者会误删新锁

- 位置：`services/token-service/src/keystore.rs:1067-1071`（`impl Drop for KeystoreLock`）
- 场景：进程 C 持锁期间被 SIGSTOP/卡顿超过 `LOCK_STALE_AFTER`（600s）；进程 A 判定锁 stale 并回收、`create_new` 建立自己的锁；C 随后完成 mutation 并 Drop —— `remove_file(&self.path)` 无条件删除，删掉的是 **A 的锁**；进程 D 随即可在 A 仍持锁时获锁 → 同一时刻两个写者。
- 影响：锁语义失效、并发 `LockBusy`/`Stale` 保护被削弱；数据不丢（指纹 CAS 在锁内仍拦截交错写），但"唯一并发机制"的声明被打破。
- 修复建议：acquire 时生成随机 token 写入锁文件，Drop 前读回比对，只删除内容等于自己 token 的锁文件；不匹配则 WARN 并保留他人锁。
- 备注：触发需要 >10 分钟持锁 + 精确时序，当前单实例部署下概率极低，故 P2 而非 P1。

### F2 [P2][已修复] stale 锁判定只看 mtime，不校验 pid 存活

- 位置：`services/token-service/src/keystore.rs:1073-1081`（`lock_is_stale`）；锁文件写入了 `pid={}` 却未被利用（`1033`）
- 影响：持锁进程挂起（SIGSTOP/调试断点/宿主机冻结）超过 600s 后，其**合法锁**被误回收，与 F1 复合放大并发窗口。
- 修复建议：unix 上回收前检查 `/proc/<pid>` 是否存在（锁文件已含 pid）；不存在才允许回收；mtime 判定保留为第二条件。同时建议把 `LOCK_STALE_AFTER` 参数化（配合 F14 的测试缺口）。

### F3 [P2][已修复（运维方案）] PR #7 生成的备份在 PR #9 下不可恢复（升级迁移注意项）

- 位置：`services/token-service/src/keystore.rs:710-716`（拒绝无 envelope 的裸 keystore 备份）；PR #7 的 `write_backup` = 裸 `serde_json::to_string_pretty(&self.data)`（`51fb12ab` 版本）
- 影响：从含 PR #7 的版本升级到本 PR 后，**升级前用旧 keytool 做的所有备份永久不可恢复**（版本升级不可逆地改变备份格式）。错误信息与 runbook（"旧版裸 keystore 备份 → 用当前版本 keytool backup 重新生成"）已给出路径，但若运维不在升级当天重做备份，灾难恢复能力存在空窗。
- 修复建议（任选其一）：
  1. 操作层面：在 runbook 升级清单中加入硬性步骤"升级后立即 `keytool backup` 重做备份并核验 `keytool restore --dry-run`"（当前 runbook 未把这一步列为升级必做项）；
  2. 代码层面：对无 envelope 的裸 keystore 备份走"接受但补包信封"的一次性迁移（不推荐：无校验和的备份静默接受违背 fail-closed 初衷）。
- 定级理由：开发阶段无生产数据 → 实际影响低；但这是审查点 10 的直接命中项，必须显式记录。

### F4 [P2][已修复] `write_backup` 不持锁 + 固定 `.tmp` 文件名 → 并发备份可产生损坏文件

- 位置：`services/token-service/src/keystore.rs:605-622`（`write_backup_inner` 直接 `atomic_write`，无 `KeystoreLock`）；`keystore.rs:1102-1104`（tmp 文件名 = 目标名 + `".tmp"`，全局固定）
- 场景：两个 `keytool backup`（或未来在线备份端点）对同一 `--out` 并发执行 → 两个进程对**同一个 tmp 文件**交错 `write_all` → rename 后的备份内容混杂损坏。restore 会被 checksum 拒绝（不会静默接受坏备份），属可用性问题而非安全问题；但报错形态（checksum mismatch）会误导排障方向。
- 修复建议：`write_backup` 全程持 keystore 锁（与 save 一致）；tmp 文件名加入 pid/uuid 后缀，顺带消除 snapshot/backup 间潜在的 tmp 冲突。

---

## P3

### F5 [P3][已修复] tmp 文件以默认 mode 写入后再 chmod 0600（短暂宽松窗口）

- 位置：`services/token-service/src/keystore.rs:1107-1119`（`File::create` → `write_all` → `set_permissions(0o600)`）
- 影响：私钥内容先以 `0644 & umask` 落盘再收紧；因父目录强制 0700，暴露面仅限同 uid 进程，实际风险极低。
- 建议：unix 上用 `OpenOptions::new().mode(0o600).create(true).truncate(true)` 一步创建受限文件。

### F6 [P3][已修复] `atomic_write` 会把相对路径场景下的 CWD chmod 0700

- 位置：`services/token-service/src/keystore.rs:1092-1101`（parent 为空时 fallback `Path::new(".")`，随后无条件 `set_permissions(parent, 0o700)`）
- 影响：`keytool backup --keystore keystore.json --out backup.json` 这类相对路径调用会把当前工作目录改成 0700，覆盖管理员原有权限。
- 建议：仅当目录是本次新建（或明确属于 keystore 目录）时才 chmod；对 `.` 直接跳过 chmod。

### F7 [P3] rename 后未 fsync 父目录（crash consistency）

- 位置：`services/token-service/src/keystore.rs:1120-1126`
- 影响：断电极端场景下文件内容已 fsync 但目录项可能未持久，rename 可能回滚为旧 keystore。容器 journal FS 上风险低；密钥持久化属严肃场景，补一行 `File::open(parent).sync_all()` 成本极低。
- 建议：`atomic_write_bytes` 成功后 fsync 父目录。

### F8 [P3] 审计写入失败不反馈到 admin API 响应

- 位置：`services/token-service/src/audit.rs:210-229`（`record` 仅 ERROR 日志 + counter）；`services/token-service/src/routes/keys_admin.rs`（响应体无审计状态字段）
- 影响：调用方（自动化脚本）无法感知"本次轮换已生效但未落审计"。
- 建议：`AuditLedger::append` 的结果透传到 handler，在响应中加 `audit_persisted: bool` 或非标准 header；保持 2xx 不变（策略是不回滚）。

### F9 [P3] 审计 detail 写入 keystore SHA-256 摘要（信息面最小化）

- 位置：`services/token-service/src/keystore.rs:732-738`（`BackupIntegrity` 错误含 recorded/computed 两个 hex）→ 经 `restore_impl` 失败分支进入 ledger detail（`684-694`）
- 影响：computed 值是当前活 keystore 的摘要。不可逆、对 RSA 无离线攻击价值，**非密钥泄漏**；但审计文件的最小信息面原则下建议不落摘要。
- 建议：错误信息只保留 "checksum mismatch" 事实 + 备份路径，摘要写入 tracing（本地排障用）而非持久 ledger。

### F10 [P3] 备份信封无 HMAC——"防篡改"仅指完整性，不防恶意重写

- 位置：`services/token-service/src/keystore.rs:124-131`（`BackupEnvelope`）
- 说明：能写备份文件的攻击者可同时改 `keystore` 并重算 `keystore_sha256`。这超出威胁模型（同权限攻击者可直接改活 keystore），非缺陷；但 runbook 表述（"备份被截断/篡改"）应避免让运维误以为信封可对抗主动攻击者。可选增强：密管侧存放备份的 SHA-256 清单做带外比对。

### F11 [P3][已修复] restore 读回验证较弱（active_kid + key 数量）

- 位置：`services/token-service/src/keystore.rs:774-787`
- 影响：理论上可能通过"key 数相同但内容不同"的验证（锁内单写者下不可能发生，属纵深防御强度问题）。
- 建议：读回后比较 `file_fingerprint` 与预期 `sha256(json)` 全量一致。

### F12 [P3][已修复（显性化）] total-loss restore 验证失败时无回滚目标，磁盘状态与返回值矛盾

- 位置：`services/token-service/src/keystore.rs:746-761`（`snapshot=None`）+ `795-810`（回滚仅在 `Some` 时执行）
- 影响：keystore 不存在时的 restore 若读回验证失败，磁盘已写入 restored 内容但命令返回 Err——语义为"恢复已生效但报错"，运维可能据此重试或误判现场。
- 建议：错误信息显式注明"本次恢复已写入磁盘但验证失败，未回滚（无 pre-restore 状态）；重试前先 `keytool inspect`"。

### F13 [P3] `refresh_runtime_state` 失败走 `expect` panic

- 位置：`services/token-service/src/routes/keys_admin.rs:276-284`
- 影响：磁盘已持久化、审计已成功之后，JWKS/issuer 内存刷新失败会 panic 崩溃进程；重启后自愈（从磁盘加载新 key），但对外表现为一次性 500/断连。
- 建议：降级为 ERROR 日志 + 触发后台重试（或直接返回 500 并提示"磁盘已变更，重启生效"），避免整进程崩溃。

### F14 [P3] 测试缺口清单

已覆盖（66 单测，本机实测全绿）：stale 快照、锁竞争 fast-fail、checksum/version/legacy 拒绝、回滚（注入失败）、快照存在性、审计无密钥、legacy import、半缺/损坏 PEM、权限 fail-closed、迁移 kid 兼容。

未覆盖，建议补齐（与 F1/F2/F4/F3 对应）：

1. stale-lock 回收路径：`LOCK_STALE_AFTER` 参数化后用 filetime 回拨 mtime 测试"回收→成功获锁"；
2. 锁 token 不匹配时 Drop 不删他人锁（F1 的回归测试）；
3. 并发 backup（F4）：两线程同 `--out`，断言要么成功要么明确报错、文件必为完整合法信封；
4. 双进程并发首启 init：两个 `generate_fresh` 同路径，断言一个成功一个 `Stale`、磁盘只留一把密钥；
5. ledger 坏行场景（见 F15 关联）：`read_all` 对损坏行的行为；
6. `expect_active_kid` mismatch 的审计断言（routes 层 record 无测试）；
7. total-loss restore 验证失败路径（`snapshot=None` 分支）。

### F15 [P3] ledger 单行损坏使整个账本不可读

- 位置：`services/token-service/src/audit.rs:232-245`（`read_all` 任一行 parse 失败即整体 `Err`）；`audit.rs:189-204`（append 不持锁，依赖 O_APPEND 单次 write 的原子性；`write_all` 部分写被抢占时理论可产生交错行）
- 影响：一条坏记录 → `keytool audit` 完全不可用（排障时最需要它的时刻）；并发写（service + keytool 同时运行是设计允许的）交错概率低但存在。
- 建议：`read_all` 跳过解析失败的行并返回 `(records, bad_lines)` 供调用方 WARN；append 保持单次 `write`（当前实现已是一次 `write_all`，行数百字节级，实际风险可接受）。

---

## 定级说明

- 无 P0：未发现密钥泄漏路径、未发现可绕过校验的路径、无数据丢失路径。
- 无 P1：F1/F4 触发概率受单实例部署与毫秒级操作时长压制，且均有数据安全兜底（指纹 CAS / checksum），故定 P2。
- F3 定 P2 而非 P3：属于"升级即不可逆改变备份可用性"的操作迁移点，需要进 runbook 与升级 checklist。

---

## P2 修复摘要（2026-11-23 remediation，详情见 P2-FIX-REPORT.md）

| # | 状态 | 修复方式（一句话） |
|---|---|---|
| F1 | ✅ 已修复 | 锁文件带 UUID v4 owner token；Drop 删除前重读并核验 owner，不匹配/损坏/缺失一律不删 |
| F2 | ✅ 已修复 | 回收判定从「mtime>600s」改为「内核锁可获取」（Unix flock / Windows share_mode(0) 独占打开）——进程崩溃内核自动释放锁，天然免疫 PID 重用；拿不到内核锁即 fail-closed 等待至超时 `LockBusy` |
| F3 | ✅ 已修复（运维方案） | runbook §4.1 新增升级前后强制 checklist：升级后必须用新 keytool 重做信封备份 + 校验 checksum + 演练验证，完成前不得删除旧备份 |
| F4 | ✅ 已修复 | backup 全程持同一把跨进程锁；快照在锁内**从磁盘重载并校验**（内存陈旧不再进入备份）；tmp 文件唯一名（uuid 后缀）+ 失败清理 |
| F5 | ✅ 已修复 | tmp/锁文件 unix 上以 `OpenOptions::mode(0o600)` 直接创建，消除 0644 窗口 |
| F6 | ✅ 已修复 | parent 为 `.` 时跳过 chmod 0700 |
| F11 | ✅ 已修复 | restore 读回验证升级为全量 checksum 比对（与备份信封 `keystore_sha256` 一致） |
| F12 | ✅ 已修复（显性化） | 无快照可回滚时 ERROR 日志显式声明"现场处于 restored 状态，需人工确认"；runbook §6 增加对应条目 |
| F7/F8/F9/F10/F13/F14(部分)/F15 | ⏸ 未修复 | 与本次 F1/F2/F4 无直接耦合，按最小化修改原则留在后续（见 P2-REMEDIATION-FINAL.md §遗留风险） |

F14 测试缺口的状态：1（stale 回收路径）已被「内核锁接管」设计取代并有 3 个新测试覆盖；
2（Drop token 不匹配）已覆盖；3（并发 backup）已覆盖；4/5/6/7 未变（其中 7 的语义已由
F12 显性化方案部分覆盖——测试仍缺，保持未修复标注）。
