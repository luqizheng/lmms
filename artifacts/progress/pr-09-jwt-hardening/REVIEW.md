# PR #9 正式代码审查 — JWT Key Lifecycle Operations Hardening

- 审查时间：2026-11-23
- PR：`luqizheng/lmms#9`（`feat(token): JWT key lifecycle operations hardening`）
- 审查 head：`652a9bfa85dbcf8e45f479a188845346bb229b10`（与远端一致）
- base：`main` @ `5b675086`（= v1.8.0 tag）
- 审查方式：阅读完整 diff（`git diff 5b675086..652a9bfa`）与全部相关调用链源码（`keystore.rs` 1746 行、`audit.rs` 337 行、`keytool.rs` 202 行、`keys_admin.rs` 325 行全文），非仅依据 PR 描述；并在本机 host 工具链实际运行单测验证。
- 本审查为独立复核，不依赖该 PR 自带的 INVESTIGATION.md / REPORT.md 结论。

## 变更面

| 文件 | 性质 |
|---|---|
| `services/token-service/src/audit.rs`（新增，337 行） | 审计账本（JSONL append-only） |
| `services/token-service/src/keystore.rs`（+817/−45） | 跨进程锁、写指纹 CAS、备份信封、restore 安全序列 |
| `services/token-service/src/bin/keytool.rs`（+58/−8） | 新增 `audit` 子命令；全部变更命令接审计 |
| `services/token-service/src/routes/keys_admin.rs`（+29/−7） | admin 端点接审计上下文（actor=JWT sub + trace_id）、expect_active_kid mismatch 审计 |
| `.gitignore`（+4） | ledger / pre-restore 快照 / lock 文件 |
| runbook / README / spec / openapi version bump | 文档与生成物 |

## 总评

**达到合并条件（有条件：无 P0/P1；4 个 P2 均不阻断，建议合并后短期跟进其中 2 个）**。

安全核心全部成立：私钥材料结构上进不了审计账本；陈旧快照覆盖被「锁内指纹 CAS」真实防住；restore 序列完整（校验→锁→快照→原子写→读回→回滚）；备份信封能拒绝篡改与不兼容版本；文件权限 0600/0700 正确。审计写入失败有明确、可观测的策略（不回滚 + ERROR 日志 + Prometheus 计数器 + `/metrics` 暴露 + runbook 告警指引）。

本机实测：`cargo test -p token_service --lib`（host MSVC 工具链，`SWAGGER_UI_DOWNLOAD_URL=file://...`）→ **66 passed / 0 failed**（22.9s）。

远端 CI：该 head 未产生 GitHub Actions check-runs（仓库 CI 主要在 GitLab；PR #8 分支的同类 validate job 为绿）。

## 逐项审查结论（对应任务要求的 10 个审查点）

### 1. 跨进程文件锁 — 通过，2 个并发缺陷（P2）

实现：`create_new` 原子建锁（`keystore.rs:1026-1035`）+ Drop 删锁（`1067-1071`）+ 10 分钟 mtime 判 stale 回收（`1037-1046`、`1073-1081`）+ 5 秒等待后 `LockBusy`。

- 竞态：`create_new` 是原子原语，无双重获锁窗口；`save`/restore 的「指纹比较 + 原子写」都在锁内，CAS 语义成立。
- 异常退出：SIGKILL 后残留锁由 10 分钟 stale 回收兜底；runbook 有对应条目。
- **缺陷（F1，P2）**：Drop 无条件 `remove_file` —— 持锁进程被判定 stale 后若迟迟返回，其 Drop 会删除**别人新建的锁**，令第三进程可并发获锁，锁语义失效（数据安全由指纹 CAS 兜底，不丢数据但保护被削弱）。
- **缺陷（F2，P2）**：stale 判定只看 mtime，不校验 pid 存活（锁文件里写了 `pid=` 却没用上）；持锁进程长期挂起时合法锁会被误回收。
- 影响面备注：残留锁期间重启服务，`init` 对已存在 keystore 走 `load()`（不写盘）不受影响；仅 first-boot/import 的写路径会在 5s 后 `LockBusy` 失败，属 fail-closed，可接受。

### 2. SHA-256 指纹防陈旧覆盖 — 通过

- `load` 记录磁盘字节 SHA-256（`keystore.rs:227`）；`persist_locked` 在锁内重读指纹、不一致即 `Stale`（`546-560`）；所有 keystore 写路径（save/restore）均持锁；backup/snapshot 写的是**其他文件**，不破坏前提。
- 双进程并发 `init`（首启生成不同密钥）也被覆盖：后写者 `fingerprint=None` vs 磁盘 `Some` → `Stale`。
- 测试 `stale_snapshot_cannot_overwrite_a_newer_rotation`（`keystore.rs:1566-1586`）实证。
- 结论：**陈旧快照无法覆盖新轮换**，机制正确。

### 3. 审计账本覆盖成功/失败路径、不含密钥 — 通过

- rotate/retire/remove/backup/restore 双路径均 `record`（`keystore.rs:344-497、570-603、655-696`）；retention 拒绝、LockBusy、Stale、expect_active_kid mismatch（`keys_admin.rs:148-155`）等失败都落账。
- `AuditPayload` 结构上无密钥承载字段（`audit.rs:100-110`）；全部 detail 调用点核查无 PEM/密钥内容；测试断言 ledger 不含 `BEGIN`/`PRIVATE KEY`/私钥原文（`keystore.rs:1521-1561`）。
- 唯一信息面扩大（F9，P3）：`BackupIntegrity` 错误把当前 keystore 的 SHA-256 摘要写进审计 detail（`keystore.rs:732-738`）。摘要不可逆、对 RSA 密钥无离线攻击价值，非泄漏，但建议最小化。

### 4. 审计写入失败的策略 — 通过，明确且可观测

- 策略：**best-effort，绝不回滚已生效的 key 变更，但绝不静默**（`audit.rs:207-229`）：ERROR 日志 + `token_keystore_audit_write_failures_total` Prometheus 计数器；`/metrics` 已暴露（`main.rs:175`）；runbook 明确"任何非零值告警"。
- 局限（F8，P3）：admin API 的 HTTP 响应不含审计失败提示，调用方无从感知；keytool 离线场景只有 stderr。

### 5. 备份信封拒绝篡改与不兼容版本 — 通过

- restore 链：无 envelope 的 PR #7 裸 keystore 备份 → `BackupVersion` 明确拒绝并给出迁移指引（`keystore.rs:710-716`）；`backup_format_version != 1` → 拒绝（`723-729`）；`keystore_sha256` 不匹配 → 拒绝（`730-738`）；随后全量 `validate_data`（PEM 解析 + 公私钥配对 + 结构校验，`846-899`）。
- 边界说明（F10，P3）：SHA-256 是完整性校验、无 HMAC——能改 keystore.json 的攻击者本可直接改活文件，重算信封校验和不构成新增攻击面；与威胁模型一致，但文档不应宣称"防恶意篡改"。
- 测试：checksum mismatch / version 99 / legacy bare 三个拒绝路径均有（`keystore.rs:1614-1653`）。

### 6. Restore 安全序列 — 通过，2 个 P3

顺序（`keystore.rs:698-819`）：读备份 → envelope 版本 → checksum → 结构校验 → **取锁** → 读活文件原字节写 pre-restore 快照（0600）→ 锁内 `persist_locked`（含 Stale guard）→ **读回验证**（`load()` 全量校验 + active_kid/key 数比对）→ 失败回滚到快照并更新指纹。测试覆盖成功与注入失败两条路径（`1656-1734`）。

- F11（P3）：读回验证只比 `active_kid` + key 数量，未比全量 SHA-256（单进程锁内写使差异不可能，但校验强度偏弱）。
- F12（P3）：total-loss 场景（活文件不存在）验证失败时无快照可回滚，磁盘留下 restored 内容但命令报错——语义为"恢复成功但报错"，错误信息未说明该状态。

### 7. 文件权限 — 通过，2 个 P3

- keystore/ledger/backup/snapshot 均 0600，目录强制 0700（`keystore.rs:1096-1119`、`audit.rs:181-199`）；启动时 `mode & 0o077 != 0` fail-closed（`1134-1149`）；`*.backup.json` 与 runtime 文件已 gitignore。
- F5（P3）：tmp 文件先以默认 mode（0644 & umask）写完再 chmod 0600——存在短暂宽松窗口；因目录 0700 仅同 uid 可见，实际风险极低。建议 `OpenOptionsExt::mode(0o600)`。
- F6（P3）：`atomic_write` 对相对路径目标会 `create_dir_all(".")` 并把 **CWD chmod 0700**（`1093-1101`）——keytool 以相对路径运行时的副作用。

### 8. 测试未覆盖的异常路径 — 详见 FINDINGS F14

覆盖面优秀（66 个单测含并发、回滚、注入失败、权限、迁移）；缺口集中在：stale-lock 回收路径（阈值硬编码 600s 无法测）、并发 backup 竞态、ledger 坏行、双进程并发首启、admin expect-mismatch 的审计断言、`refresh_runtime_state` panic 路径。

### 9. 与 PR #7 的重复/冲突 — 无重复，一处刻意破坏性变更

- 审计账本、锁、指纹、信封均为新增能力，非重复实现；`write_backup` 签名加 `ctx` 属同 PR 内闭环修改（keytool/keys_admin 同步更新）。
- 唯一冲突点：**PR #7 生成的裸 keystore 备份在 PR #9 下不可恢复**（F3，P2）——刻意设计（无校验和的备份拒绝恢复），有明确错误信息与迁移指引，runbook 已写"用当前版本 keytool backup 重新生成"。升级操作上必须在升级后**重新做一次备份**。

### 10. keystore 文件兼容性迁移风险 — 低，一个操作注意项

- `KEYSTORE_VERSION` 仍为 1，`KeyStoreData` 结构未变：PR #7 写下的 `keystore.json` 在 PR #9 下直接可读、可继续轮换。**无需数据迁移**。
- 风险仅存在于备份文件（见第 9 点 F3）：升级前用 PR #7 keytool 做的备份升级后作废。开发阶段无生产数据，实际影响低。

## 结论

| 维度 | 判定 |
|---|---|
| P0 / P1 | 无 |
| P2 | 4 个（F1 锁 Drop 误删、F2 stale 判定无 pid 校验、F3 PR#7 备份格式不兼容、F4 并发 backup 无锁） |
| P3 | 10 个（见 FINDINGS.md） |
| 单测 | 66 passed / 0 failed（本机实测） |
| **合并判定** | **达到合并条件**。P2 中 F4（并发 backup）与 F3（升级后重做备份的运维提示）建议合并前顺手处理或紧跟修复；F1/F2 可合并后跟进（触发概率极低且有指纹 CAS 兜底）。 |

问题明细与修复建议见同目录 `FINDINGS.md`。
