# PR #9 调查报告：JWT 密钥生命周期运维加固

- 调查日期：2026-11-23
- 基线：`github/main` @ `5b675086597e001e794a4c2f3a3575a431af6cc9`（含 PR #7 `feat(token): persist JWT signing keys ... (#7)`）
- 分支：`feat/jwt-key-operations-hardening`
- 调查方式：读源码 + 配置 + 部署编排 + 现有测试；不预设结论

## 1. 现状证据（PR #7 已交付的能力）

| 能力 | 实现位置 | 关键行为 |
|---|---|---|
| 密钥存储 | `services/token-service/src/keystore.rs` | 单一真相源 `keystore.json`（默认 `/app/keys/keystore.json`）；条目含 `kid/status/created_at/retired_at/private_pem/public_pem` |
| 启动解析 | `KeyStore::init`（keystore.rs:100-139） | keystore 存在→加载；只有旧 PEM 对→一次性导入（kid=`default`）；都没有→生成；半缺/损坏→fail-closed |
| 原子写 | `atomic_write`（keystore.rs:554-593） | 同目录 `.tmp` + `write_all` + `sync_all` + `rename`；unix 文件 0600 / 目录 0700，加载时校验权限 |
| 轮换 | `KeyStore::rotate`（keystore.rs:267-298） | 新钥 active；原 active → `retiring`（仍在 JWKS）；原子落盘 |
| 退役 | `KeyStore::retire`（keystore.rs:303-321） | 仅 `retiring`→`retired`；移出 JWKS |
| 删除 | `KeyStore::remove`（keystore.rs:327-359） | 仅 `retired` 且 `now-retired_at ≥ TOKEN_JWT_MIN_RETENTION_SEC` |
| 验签查钥 | `routes/jwks.rs`、`keystore.rs::public_entries` | JWKS 返回 active+retiring；下游 `JWKSCache` 定时刷新 |
| 在线管理面 | `routes/keys_admin.rs` | `/admin/keys`、`/rotate`、`/{kid}/retire`、`DELETE /{kid}`，service-jwt `aud=token-service` 准入 |
| 离线工具 | `src/bin/keytool.rs` | inspect/rotate/retire/delete/backup/restore；镜像内 `/app/keytool` |
| 备份 | `write_backup`（keystore.rs:376-381） | 直接把 `KeyStoreData` 原样 pretty JSON 写到目标路径（0600） |
| 恢复 | `restore_from_backup`（keystore.rs:387-411） | 读→`serde_json` 解析→`validate_data`→直接覆盖 live keystore |

`validate_data`（keystore.rs:438-491）已覆盖：版本号、非空、`active_kid` 存在且为 active、kid 唯一、每个 PEM 可解析、私钥/公钥匹配、retired 必有 `retired_at`。

已存在的单测（keystore.rs `mod tests`）覆盖：首启生成、重启不变、旧 JWT 存活、轮换后旧 token 可验、retire/remove 保留期、损坏 fail-closed、权限 fail-closed、备份恢复后旧 token 可验、损坏备份拒绝恢复、旧 PEM 导入兼容。**这些是 PR #7 成果，PR #9 不重复实现。**

## 2. 部署模型证据（决定多实例方案）

- `infrastructure/lmms-dev/docker-compose.yml:86-...`、`examples/docker-deploy*/docker-compose-services.yml:94-118`：
  token-service **单容器**，keys 挂 named volume `token-keys:/app/keys`（`examples/docker-deploy-full/...:116-118,297-298`）。
- 全仓 compose 无 `deploy.replicas`，无 token-service 多副本编排。
- 结论：**当前交付拓扑 = 单实例 + 本地/命名卷**。多实例不是现有部署事实，不应为理论多实例引入分布式锁/协调服务。

## 3. 实际发现的问题（按证据）

### P1 审计缺失（最高优先级）

- `services/token-service/` 全目录 grep `audit|operation_log` 仅命中测试数据与 bootstrap owner 字符串，**没有任何密钥生命周期审计写入**。
- 现有统一审计链路：`audit-service` 通过 MQ 消费 `iam.#` / `pmd.#`（`services/audit-service/src/consumer.rs:51-61`）落库 `audit_events`（`migrations/mysql/V001__init.sql:54-66`，字段 `event_id/event_type/occurred_at/trace_id/producer/payload_json`）。
- token-service 目前**没有** outbox 表、没有事件生产、没有 RabbitMQ 依赖（`services/token-service/Cargo.toml`、`migrations/` 仅 V001-V003）。
- 结论：轮换/退役/删除/备份/恢复全部无持久化审计；密钥泄露后无法回答"谁在何时动了哪把 key"。

### P1 备份无完整性校验、无可识别的格式版本

- `write_backup` 直接序列化 `KeyStoreData`，无校验和、无备份格式版本、无生成时间/生产者。
- `restore_from_backup` 只做 `version == KEYSTORE_VERSION`（数据格式版本）与 PEM 校验；截断/篡改但仍是"合法 JSON 且 PEM 自洽"的备份（例如被拼接成另一套 key）无法与"正确备份"区分。运输/存储损坏只能靠 PEM 解析偶然失败来拦截。
- 结论：缺少显式完整性校验（checksum）与备份格式版本兼容性检查。

### P1 恢复失败可能破坏原状态 / 无恢复前保护

- `restore_from_backup` 校验通过后直接 `ks.save()` 覆盖 live keystore，**不保留恢复前快照**。
- 若备份语义上错误（例如旧备份把已轮换掉的旧钥集整套覆盖回来），线上正在使用的 active key 会被替换，且无自动回退材料；操作者只能依赖手动备份。
- 恢复成功后的读回验证缺失（写盘后没有再从磁盘加载确认）。

### P2 并发/外部写入可造成静默丢失更新

- `atomic_write` 只保证"不出现半写文件"，**不防丢失更新**：进程 A 加载 keystore 到内存→外部 `keytool`（或另一实例）改写文件→A 保存时用自己的内存快照整体覆盖，外部改动丢失（例如刚轮换出的新钥被抹掉 → 新签发的 token 立刻无法验签）。
- `keytool`（`src/bin/keytool.rs:61-67`）与在线服务无任何跨进程互斥；`KeyStore` 也无"磁盘已被他人改动"的检测。
- 多实例：`TokenServerState.keys` 在启动时加载一次（`main.rs:100-156`），`routes/jwks.rs` 读内存；另一实例的 rotate 不会传播，轮换后两实例各自签发不同 kid 的 token、JWKS 取决于请求命中哪个实例。**但现有部署是单实例**，因此按"记录限制 + 让违规并发显式失败"处理，而非引入分布式协调。

### P2 运行期状态与磁盘可能短时不一致（重启可恢复）

- `routes/keys_admin.rs::refresh_runtime_state` 用 `expect()` 在 JWKS 安装/issuer 换钥失败时 panic；因 keystore 已落盘，重启可恢复，故不列为必须改项，仅在文档提示。

### P3 其它（本次不改，记录）

- `keytool rotate/retire/delete` 与在线 API 能力重叠，属"停服窗口"路径；文档需明确二者互斥。
- `atomic_write` 未 fsync 父目录（rename 持久性）；影响极小，记录为已知风险。

## 4. 方案选择

### 4.1 审计

要求"复用项目已有统一审计事件模型，不另建体系"。证据表明：

1. 统一模型 = `audit_events` 行（`event_id/event_type/occurred_at/trace_id/producer/payload_json`），由 MQ 事件落库；
2. 但本 PR 必须审计的 **备份/恢复只存在于离线 `keytool`**，它按设计不连数据库；给 keytool 引入 DB/MQ 依赖会破坏离线工具的可用前提；
3. keystore 是文件状态，无法与 DB 事务共享原子性，"同事务 enqueue"在此不成立；
4. token-service 当前无 outbox/事件基础设施，新建 outbox + publisher + audit-service 新订阅面属于跨服务架构扩容，与"最小合理改动"冲突。

**决定**：在 keystore 同目录新增**追加式持久审计账本** `keystore-audit.jsonl`（0600，逐条 `fsync`），记录字段**采用统一 `audit_events` 行结构**（`event_id/event_type/occurred_at/trace_id/producer` + 非敏感 payload：actor/action/kid/result/related_kid）。它满足"持久化、不依赖进程内存、覆盖在线+离线全路径、绝不写私钥"。向 `audit_events` 的事件化投递（需 outbox + publisher + audit 订阅）列为**后续项/需人工决策**，理由与风险写入 REPORT。

禁止项落实：账本无任何 PEM/密钥原文/凭证字段；测试断言账本不含 `BEGIN ... PRIVATE KEY`。

### 4.2 多实例与并发

- 交付拓扑为单实例：**不引入分布式锁**；在文档中写明"水平扩容不受支持"及其扩展条件（改为 DB 事务存储 + 共享协调）。
- 针对**真实存在**的跨进程并发（在线服务 vs `keytool`）：新增 `keystore.json.lock`（`create_new` 互斥，超时/陈旧锁处理）+ **写前磁盘指纹校验**（加载时记录 sha256，保存前比对，不一致即 `Stale` 失败并提示重启/重试）。效果：不再静默覆盖；多实例违规并发会显式报错而非丢钥。

### 4.3 备份与恢复

- 新备份格式 `BackupEnvelope`：`backup_format_version`、`created_at`、`producer`、`keystore_sha256`、`keystore`（原 `KeyStoreData`）。
- 恢复：校验 BackupEnvelope 外层版本 → 校验 checksum → `validate_data` → 若 live keystore 存在则写**恢复前快照** `keystore.pre-restore.<ts>.json`（0600）→ 加锁 + 指纹校验下原子覆盖 → 从磁盘读回验证；读回失败用快照回滚并报错。
- 保留原"恢复后历史 token 可验"性质（由 checksum 校验过的同一套 key 保证）。

### 4.4 文档

- `docs-site/runbooks/jwt-key-operations.md`（新增运维手册）+ `services/token-service/README.md` 增量（命令必须与实现一致，不写未实现功能）。

## 5. 最小改动范围（PR #9 边界）

改：
- `services/token-service/src/keystore.rs`：审计账本、锁、指纹校验、备份信封/校验/快照/回读回滚
- `services/token-service/src/audit.rs`（新增）：账本记录结构与追加写
- `services/token-service/src/routes/keys_admin.rs`：传入 actor/trace_id，失败也审计
- `services/token-service/src/bin/keytool.rs`：actor=keytool；新增 `audit` 只读子命令；backup/restore 审计
- `services/token-service/src/main.rs`：启动/首次导入生成写审计
- 测试：`keystore.rs` 单测扩展 + `tests/keystore_admin_test.rs`（HTTP 层，需要 service-jwt 则用纯函数/直接状态测试）
- spec + runbook + README + 本报告

不改：
- 不改 key 状态机语义、不改 admin 路由形状、不引入 DB/outbox/MQ、不改部署编排、不改 main。

## 6. 明确不做的（避免过度设计）

- 不做分布式锁 / 多实例实时同步（部署非多实例；文档记录扩展条件）。
- 不做备份加密（属密钥保管体系，超出本 PR；风险在手册中提示）。
- 不把审计投递到 `audit_events`（需跨服务事件基础设施，列为待决策）。
- 不改 `refresh_runtime_state` 的 panic 语义（重启可恢复，仅文档提示）。
