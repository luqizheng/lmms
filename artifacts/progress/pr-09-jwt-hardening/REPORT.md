# PR #9：JWT 密钥生命周期运维加固 — 交付报告

- PR：https://github.com/luqizheng/lmms/pull/9
- 分支：`feat/jwt-key-operations-hardening` → `main`
- 代码修复 commit：`0481726f100610b1cefb1ae209cc9ccc1c1f8817`
  （`fix(token): close keystore lock and backup concurrency gaps`，2026-11-23 P2 remediation）
- 本文件更新于 P2 remediation 文档 commit（即推送后 PR head，SHA 见
  [`P2-REMEDIATION-FINAL.md`](./P2-REMEDIATION-FINAL.md)）
- 历史标注：初版交付时最终提交为 `39df6ce1ea6ace8a2d9984d904cc312087c6c4a6`；
  该 SHA **已不再是**最终 head（后续有 formal review commit `02721807` 与 P2 修复 commit），
  保留于此仅为历史追溯。
- 基线 main SHA：`5b675086597e001e794a4c2f3a3575a431af6cc9`
- PR 状态：**open（未合并）**
- 报告更新时间：2026-11-23（Asia/Shanghai）

## 一、结论

- 基于代码/配置/部署模型的实证调查，确认 PR #7 遗留 5 类运维风险并逐项加固；未重复实现 PR #7 已有能力。
- 后续独立复审发现 4 个 P2（锁 Drop 误删、stale 判定无 pid 校验、旧格式备份升级空窗、并发备份不一致），已于 2026-11-23 完成**代码级修复**（见 §五.5 与 [`P2-FIX-REPORT.md`](./P2-FIX-REPORT.md)）。
- 本地 `cargo test -p token_service --lib` → **79 passed / 0 failed**（修复后，Windows host）；`cargo clippy -p token_service --all-targets -- -D warnings` 无 error；5 项仓库门禁全部通过。
- GitHub Actions 结果以推送后当前 head 的实际运行为准（见 [`P2-REMEDIATION-FINAL.md`](./P2-REMEDIATION-FINAL.md)，未完成前不标注 success）。
- 未合并任何 PR、未创建 Tag、未发布任何产物、未修改 Secrets、未删除密钥、未执行不可逆 DB 操作、未修改 main。

## 二、调查报告

见 [`INVESTIGATION.md`](./INVESTIGATION.md)；设计见 [`2026-11-23-token-jwt-key-operations-hardening-design.md`](../../../docs/superpowers/specs/2026-11-23-token-jwt-key-operations-hardening-design.md)。

实证发现的真实问题：

| # | 级别 | 问题 | 证据 |
| --- | --- | --- | --- |
| P1 | 高 | 密钥生命周期操作**完全没有审计** | token-service 无任何 audit 写入路径，无 outbox/MQ/DB 事件基础设施 |
| P1 | 高 | 备份是**无 checksum、无格式版本**的裸 `KeyStoreData` | `backup_json` 直接序列化 `KeyStoreData`，恢复无法区分损坏/错误备份 |
| P1 | 高 | `restore` 直接覆盖 live keystore，**无快照/无读回校验/无回滚** | 恢复写盘后不核对结果，失败即丢失现存状态 |
| P2 | 中 | `atomic_write` 只防半写，**不防丢失更新** | 在线服务与 `keytool`（或另一实例）并发写时后写者整体覆盖，可能静默删除刚轮换的新钥 |
| P2 | 中 | 多实例状态不可共享 | 各实例内存快照互不可见；但交付拓扑为**单实例** |

## 三、设计决策及原因

1. **审计落点：本地持久化追加账本 `keystore-audit.jsonl`，结构对齐 `audit_events`。**
   - 原因：备份/恢复只在离线 `keytool` 存在，无 DB/MQ；keystore 是文件状态，无法与 DB 事务原子提交；引入 outbox 属过度工程。
   - 记录行与统一审计模型同构（`event_id/event_type/occurred_at/trace_id/producer/payload`），为后续接入 audit-service 事件管道预留；已作为需人工决策项列出。
   - 逐条 fsync，0600；写入失败打 ERROR 并累加 `token_keystore_audit_write_failures_total`。
2. **多实例：按真实架构（单实例）不引入分布式锁，改用「跨进程文件锁 + 写前指纹」让违规并发显式失败。**
   - 依据：compose 每个部署单元仅 1 个 token-service 容器（`token-keys` volume，无 `deploy.replicas`）。
   - 陈旧快照写盘返回 `Stale`（HTTP 409 / CLI 报错），绝不静默覆盖；扩展条件写入运维手册。
3. **备份/恢复：版本化信封 + SHA-256 完整性 + 恢复前快照 + 读回校验 + 自动回滚。**
   - 信封 `backup_format_version / created_at / producer / keystore_sha256 / keystore`；旧裸格式明确拒绝。
   - 顺序：信封版本 → checksum → 全量 PEM/公私钥校验 → 锁 → 快照 `keystore.pre-restore.<epoch>.json` → 指纹校验下原子覆盖 → 读回校验；失败自动回滚并报 `RestoreFailed`。

## 四、修改文件清单（相对 main，18 个文件，+1742 / −68；本报告另行追加）

| 文件 | 类型 | 说明 |
| --- | --- | --- |
| `artifacts/progress/pr-09-jwt-hardening/INVESTIGATION.md` | 新增 | 调查报告 |
| `docs/superpowers/specs/2026-11-23-token-jwt-key-operations-hardening-design.md` | 新增 | 设计文档 |
| `docs-site/runbooks/jwt-key-operations.md` | 新增 | 运维手册（轮换/退役/备份/恢复/恢复失败/多实例/审计/泄露应急/风险） |
| `docs-site/runbooks/secret-rotation.md` | 修改 | 修正 JWT 已实现状态、指向新手册 |
| `docs-site/.vitepress/config.mts` | 修改 | runbooks 侧栏挂载新手册 |
| `services/token-service/src/audit.rs` | 新增 | 审计账本与事件模型 |
| `services/token-service/src/keystore.rs` | 修改 | 写锁/指纹/备份信封/恢复快照+读回+回滚/审计接入 |
| `services/token-service/src/routes/keys_admin.rs` | 修改 | 在线操作注入审计上下文与失败审计 |
| `services/token-service/src/bin/keytool.rs` | 修改 | 新增 `audit` 子命令、各命令传审计上下文、文案更新 |
| `services/token-service/src/routes/jwks.rs` | 修改 | 测试适配新签名 |
| `services/token-service/src/lib.rs` | 修改 | 挂载 `audit` 模块 |
| `services/token-service/README.md` | 修改 | 备份/审计/并发语义说明 |
| `.gitignore` | 修改 | 忽略审计账本、快照、锁文件 |
| `services/{audit,file,iam,prue_backend,token}-service/openapi.yaml` | 修改 | 修复 main 上 workspace 1.8.0 与 spec 1.7.0 的版本漂移（再生成） |

## 五、修复内容

- **审计**：覆盖 create / import / rotate / retire / delete / backup / restore 及失败（`token.jwt_key.restore_failed` 独立事件）；记录 actor（在线=service-jwt `sub`，离线=`keytool`，首启=`system`）、action、result、kid、related_kid、trace_id、detail；**绝不含 PEM/私钥/凭证**。
- **并发一致性**：`keystore.json.lock`（持有期间保持句柄打开 + 内核级互斥锁：Unix `flock` / Windows 独占共享模式；锁文件记录 UUID v4 owner token / pid / 创建时间；持有者存活期间锁绝不被回收，接管仅发生在内核锁已释放时）+ 写前磁盘 SHA-256 指纹；陈旧写返回 `Stale`。
- **备份/恢复**：见 §三.3；损坏 checksum / 不兼容版本 / 旧裸格式均拒绝且**不触碰 live keystore**；恢复失败自动回滚、原有历史 token 继续可验签。
- **运维手册**：命令与实际 CLI/HTTP 完全一致，未写入任何未实现功能。
- **P2 remediation（2026-11-23，commit `0481726f`）**：
  1. 锁所有权：owner token + Drop 核验（F1）；
  2. 锁回收策略：内核锁接管，替代 mtime 判定（F2）；
  3. backup 持锁 + 锁内磁盘重载快照 + 唯一 tmp（F4）；
  4. runbook §4.1 旧格式备份升级强制流程（F3）；
  5. 顺带修复 P3：F5（0600 直建）、F6（`.` 不 chmod）、F11（读回全量 checksum）、F12（无回滚目标显性化）。
  详情见 [`P2-FIX-REPORT.md`](./P2-FIX-REPORT.md)。

## 六、测试命令与结果

### 当前状态（P2 修复后，fix commit `0481726f`，本机 Windows host）

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| 格式 | `cargo fmt --all -- --check` | 通过 |
| 单元测试 | `cargo test -p token_service --lib` | **79 passed / 0 failed** |
| Clippy | `cargo clippy -p token_service --all-targets -- -D warnings` | 无 error |
| 服务规范 | `node scripts/check-service-standard.js` | 通过 |
| 契约规范 | `node scripts/check-spec.js` | 通过 |
| 契约漂移 | `node scripts/contracts-gen.js --check` | 通过（in sync） |
| 表名前缀 | `node scripts/check-db-naming.js` | 通过 |
| OpenAPI 漂移 | `node scripts/dump-openapi-specs.js` + `git diff --exit-code -- 'services/*/openapi.yaml'` | 通过（无漂移） |

### 历史状态（初版交付，`39df6ce1`，66 passed）——保留作历史对照，非当前结果

| 检查 | 结果 |
| --- | --- |
| 单元测试 | 66 passed / 0 failed |
| Clippy / 5 项门禁 | 全过 |

覆盖场景（新增/强化单测，含 P2 remediation）：正常轮换；并发轮换（陈旧快照被拒且不丢钥）；退役/删除保留期；审计持久化与重启后可读；审计不含私钥原文；备份 checksum 篡改拒绝；损坏备份、不兼容版本、旧裸格式拒绝；恢复失败回滚且 live 文件字节不变；恢复后历史 token 验签；成功恢复生成恢复前快照；多实例场景（同一文件两视图 → 显式 `Stale`）；**锁所有权（Drop 只删自己 token 的锁、篡改场景双平台语义）、持有者存活不回收（伪造一天前时间戳）、死持有者/旧格式锁接管重写、rotate/backup/restore 三类并发（barrier/channel 定序、备份串行前缀断言）、同目标并发备份、备份失败不损坏既有文件**。

## 七、GitHub Actions（历史：初版交付 `39df6ce1`；当前 head 见 P2-REMEDIATION-FINAL.md）

| 工作流 | Run ID | 结果（历史 head `39df6ce1`） |
| --- | --- | --- |
| OpenAPI Generation Validation | [45130581838](https://github.com/luqizheng/lmms/actions/runs/45130581838) | success |
| Database Dialect Matrix | [45130581924](https://github.com/luqizheng/lmms/actions/runs/45130581924) | success |

## 八、未完成事项

1. **审计暂未投递 audit-service**：离线 keytool 无 DB/MQ，文件状态无法与 DB 事务原子；账本结构已对齐 `audit_events`，接入事件管道为后续项（需人工决策）。
2. **多实例实时同步不支持**：交付为单实例；水平扩容需另行设计共享状态（如 DB/外部 KV），扩展条件见手册 §1。
3. **真实在线并发压测未做**：仅在单进程测试中模拟双视图陈旧写；真实多进程竞争由文件锁覆盖但未做长稳压测。

## 九、已知风险

- 文件锁依赖本地文件系统语义；NFS 等弱一致性共享存储上 `create_new` 原子性不保证（当前部署为本地卷，不涉及）。
- 审计账本与 keystore 非同一原子事务；极端断电可能丢失最后一条审计记录（不影响密钥安全）。
- `restore` 需要停服窗口；热恢复会与实际写入竞争，故文档要求先停服。

## 十、需人工决策项

1. 是否将 `keystore-audit.jsonl` 接入 audit-service 事件管道（需为离线 keytool 设计 DAO/HTTP 投递或 sidecar）。
2. 若未来需要多实例，是否引入共享密钥存储（DB/外部 KV）及其一致性方案。

## 十一、安全边界确认

- ✅ 未合并任何 PR；PR #9 保持 open。
- ✅ 未创建任何正式版本 Tag。
- ✅ 未触发真实公开发布；未上传 crates.io / npm / Docker Hub。
- ✅ 未修改 GitHub Secrets；未使用任何真实发布凭证。
- ✅ 未删除任何现有密钥；未执行不可逆数据库操作；未修改 main。
