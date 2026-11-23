# JWT 签名密钥运维手册（token-service）

> **状态**：与实现同步（2026-11-23，PR #9 加固 + P2 修复：锁所有权/接管策略/备份一致性）
> **关联**：`docs/superpowers/specs/2026-11-23-token-jwt-key-persistence-design.md`（PR #7 基础能力）、
> `docs/superpowers/specs/2026-11-23-token-jwt-key-operations-hardening-design.md`（PR #9 审计/一致性/备份加固）
> **权威来源**：`services/token-service/README.md`（存储模型与命令细节）

本文所有命令均对应当前实现，不存在"纸面功能"。命令中的 `<token-container>` 替换为实际容器名
（如 `lmms-token-service-1`）；`/app/keys` 是 `token-keys` named volume 的挂载点。

---

## 0. 一页速查

| 操作 | 在线（推荐） | 离线（停服窗口） |
|---|---|---|
| 查看密钥 | `GET /admin/keys` | `keytool inspect --keystore /app/keys/keystore.json` |
| 轮换 | `POST /admin/keys/rotate` | `keytool rotate --keystore /app/keys/keystore.json` |
| 退役 | `POST /admin/keys/{kid}/retire` | `keytool retire --keystore … --kid <kid>` |
| 删除（保留期满） | `DELETE /admin/keys/{kid}` | `keytool delete --keystore … --kid <kid>` |
| 备份 | — | `keytool backup --keystore … --out <file>` |
| 恢复 | — | `keytool restore --backup <file> --keystore …` |
| 查审计 | — | `keytool audit --keystore … [--limit N]` |

在线管理面要求 service-jwt 且 `aud=token-service`（与其他 admin 接口一致）。

---

## 1. 部署模型与多实例要求（先读）

- 当前交付拓扑是 **单实例**：每个 compose 单元只有 1 个 token-service 容器，
  `/app/keys` 挂载 `token-keys` named volume。**水平扩容不受支持。**
- 原因（实证）：keystore 是进程内加载一次的本地文件；`JWKS`/issuer 从内存读取。
  若强行起两个副本：
  - 各自持有独立内存快照，A 轮换后 B 不知道；
  - 请求落到哪个副本，就用哪个副本的 active key 签发；JWKS 也取决命中副本；
  - 并发写入靠 **写前磁盘指纹校验** 兜底：后写者会收到 `409 Stale`
    （消息提示 "changed on disk since it was loaded"），**不会静默覆盖**，但业务上仍不可用。
- 扩展条件（满足其一才可以多副本）：
  1. 把密钥状态迁到数据库（行锁/事务）或带租约的协调服务，并改造 `JWKS` 读取为共享存储；
  2. 或明确接受"滚动更新窗口内单副本"（`replicas: 1` + `Recreate` 策略）。
- **务必遵守**：离线 `keytool` 只能在"停服 / 单副本"窗口执行；在线服务运行期间请走 HTTP 管理面。

---

## 2. 正常轮换

```
active ──rotate──▶ retiring ──retire──▶ retired ──delete(≥保留期)──▶ 删除
 (签发)            (仅验证，在 JWKS)     (不在 JWKS)
```

在线：

```bash
curl -X POST https://<gateway>/token/admin/keys/rotate \
  -H "Authorization: Bearer <service-jwt aud=token-service>" \
  -H "Content-Type: application/json" \
  -d '{"expect_active_kid":"<当前 active kid>"}'   # 可选：防止并发覆盖/陈旧视图
```

- 旧钥立即降级为 `retiring`，仍出现在 JWKS → 未过期旧 token 继续可验，**无人被登出**；
- 返回 `new_kid` / `previous_kid`；响应头部 token 立即用新钥签发；
- 若带 `expect_active_kid` 且与当前不符 → `409`，说明有人先轮换过（审计记为 failed）。

离线（停服）：

```bash
docker stop <token-container>
docker exec <token-container> /app/keytool rotate --keystore /app/keys/keystore.json
docker start <token-container>
```

建议节奏：例行 90 天一次；密钥疑似泄露立即执行（见 §7）。

---

## 3. 退役

确认目标 kid 签发的 token 已全部过期（至少超过 access TTL；refresh 场景按 refresh TTL 计算）。

```bash
# 在线
curl -X POST https://<gateway>/token/admin/keys/<kid>/retire -H "Authorization: Bearer <service-jwt aud=token-service>"
# 离线（停服）
docker exec <token-container> /app/keytool retire --keystore /app/keys/keystore.json --kid <kid>
```

- 只能退役 `retiring` 的钥；`active` 直接退役会 `409`（防止把唯一签发钥摘掉）；
- 退役后该 kid 移出 JWKS；下游 `JWKSCache` 刷新周期（默认 1h）内仍可能接受旧 token —
  这是预期行为，退役应在 token 全部过期后进行。

删除：仅 `retired` 且已过 `TOKEN_JWT_MIN_RETENTION_SEC`（默认 2592000s = 30 天）才允许；
未到期返回 `409`，重新计算时间即可。**删除不可逆**，删除前务必已有可用备份。

---

## 4. 备份

```bash
# 容器内生成，再移出到密管存储
docker exec <token-container> /app/keytool backup \
  --keystore /app/keys/keystore.json --out /app/keys/keystore.backup.json
docker cp <token-container>:/app/keys/keystore.backup.json /secure/vault/keystore-$(date +%Y%m%dT%H%M%S).backup.json
docker exec <token-container> rm -f /app/keys/keystore.backup.json
```

- 备份是**带信封的格式**：`backup_format_version` / `created_at` / `producer` /
  `keystore_sha256`（对 keystore 规范化 JSON 的 SHA-256）/ `keystore`；
- 备份与 rotate/restore 走**同一把跨进程锁**，快照在锁内**从磁盘重新加载并校验**后生成：
  即使 keytool 进程内存里的视图过时，备份也反映锁保护时刻的真实磁盘状态，不会产生
  撕裂/陈旧快照；
- 文件 0600，`*.backup.json` 已 gitignore；**含全部私钥，严禁提交 Git、严禁进工单附件**；
- 每次 rotate 后 + 每周例行备份；备份本身**未加密**（见 §8 风险）；
- `keytool backup` 完成时会打印 `keystore_sha256`，请与密管存储中登记的值核对后再删除容器内副本。

### 4.1 PR #7 旧格式备份升级（必读）

**PR #7 时代生成的"裸 keystore"备份**（文件直接是 `KeyStoreData` JSON，无
`backup_format_version`/`keystore_sha256` 信封）**不受当前 `keytool restore` 支持**：
restore 会在第 1 步即拒绝并报 `unsupported backup format`（`BackupVersion`，错误消息
含 "legacy bare-keystore backup"）。这是有意的 fail-closed 设计——旧格式没有完整性
校验，静默兼容它会绕过新版本的防篡改检查。

**从运行 PR #7 的版本升级到 PR #9 及以后时，按以下顺序执行：**

升级前（旧版本仍在正常运行时）：

1. 确认旧版本服务正常：`GET /health` 通过，`keytool inspect` 能列出密钥；
2. 记录核对信息（**只记 kid/数量/时间戳，绝不记录密钥内容**）：
   - `keytool inspect --keystore /app/keys/keystore.json` 输出的 active kid 与密钥数量；
   - 各 kid 的 status（active/retiring/retired）与 created_at；
3. 确认存在可用的旧备份并**在安全环境（密管存储）保留一份**；
4. 用旧版本自身的恢复机制或演练环境验证旧备份可读（不要求能被新版本 restore）。

升级后（强制步骤，未完成前不得删除旧备份）：

1. 服务升级后确认正常启动、`keytool inspect` 的 active kid 与密钥数量与升级前记录**一致**；
2. **用升级后的 keytool 重新生成信封格式备份**：`keytool backup --keystore … --out <file>`；
3. 核对命令输出的 `keystore_sha256`，并在演练环境验证新备份可被当前版本
   `keytool restore` 识别（restore 到临时路径，不要覆盖生产 keystore）；
4. 将新备份保存到密管存储并登记 checksum；
5. **完成第 3-4 步并验证通过之前，绝不删除旧格式备份**——它是升级失败时唯一的回退材料。

升级失败回退：若新版本无法启动或 keystore 校验失败，回到旧版本镜像（compose 改回旧
image tag 后 `docker compose up -d`），用升级前记录的 active kid 核对 keystore 未被改动；
若 keystore 本身损坏，在旧版本上用旧备份恢复（旧版本支持旧格式），再重新走升级流程。

---

## 5. 恢复

```bash
docker cp /secure/vault/keystore-XXXX.backup.json <token-container>:/tmp/
docker stop <token-container>
docker exec <token-container> /app/keytool restore \
  --backup /tmp/keystore.backup.json --keystore /app/keys/keystore.json
docker exec <token-container> rm -f /tmp/keystore.backup.json
docker start <token-container>
```

恢复的内部顺序（全部通过才写盘）：

1. 解析信封 + 检查 `backup_format_version`（不支持，含 PR #7 旧格式裸备份 → 拒绝，`BackupVersion`，重新用当前版 keytool 生成，见 §4.1）；
2. 计算 `keystore_sha256` 并与备份记录比对（不一致 → 拒绝，`BackupIntegrity`）；
3. 全量 `validate_data`（PEM 可解析、公私钥匹配、active 存在、kid 唯一…）；
4. 取 `keystore.json.lock` 跨进程锁；
5. 写**恢复前快照** `keystore.pre-restore.<epoch>.json`（0600，同目录）；
6. 指纹校验下原子覆盖（若期间有他人写入 → `Stale`，拒绝并保留现场）；
7. 从磁盘读回并做**全量 checksum 比对**（与备份信封中 `keystore_sha256` 一致）——失败则用快照自动回滚并报 `RestoreFailed`；若无恢复前快照（原 keystore 不存在）则无法回滚，日志会以 ERROR 显式说明现场处于 restored 状态，需人工确认。

恢复后重启，备份内包含的密钥所签发的历史 token 恢复可验证。

---

## 6. 恢复失败后处理

| 现象 | 含义 | 处理 |
|---|---|---|
| `backup integrity check failed` | 备份被截断/篡改/传输损坏 | 换另一份备份；重新从密管取原件校验 sha256 |
| `unsupported backup format`（含 "legacy bare-keystore"） | PR #7 旧格式裸 keystore 备份 | 按 §4.1 升级流程：升级后在当前版本重新 `keytool backup`；不要试图绕过校验 |
| `restore failed and the previous keystore was restored` | 写盘后读回 checksum 不一致，已自动回滚 | 现场未破坏；检查磁盘健康/权限后重试或换备份 |
| `restore failed`（日志含 "NO pre-restore snapshot"） | 原 keystore 不存在导致无回退材料，现场处于 restored 状态 | 立即人工核对 `keytool inspect`；确认无误后重启服务，否则换备份重新 restore |
| `keystore changed on disk since it was loaded` | 恢复期间有并发写入 | 停掉其他写入者（另一 keytool/副本）后重试 |
| `another key operation is in progress` | 锁被占用 5s | 确认无并发操作。锁文件由内核锁保护：持有者存活期间锁不会被回收；持有者退出/崩溃后下一次操作自动接管，**不要手动删除 `keystore.json.lock`**（若确需手动清理，先 `kill` 确认没有 keytool/服务进程在跑） |

恢复前快照 `keystore.pre-restore.<epoch>.json` **不要删除**：它保存了恢复前的完整密钥集，
是"恢复选错备份"时唯一的回退材料。

---

## 7. 密钥泄露应急处置

1. **立即轮换**（在线 `POST /admin/keys/rotate`），确认新 kid 已生效；
2. 泄露钥若只用于签发：可**提前退役**（不再出现在 JWKS）→ 所有相关 token 在外层
   `JWKSCache` 刷新后失效；同时清理 Redis 中该 kid 相关会话（如有）；
3. 泄露钥仍在 `retiring` 时**不要删除**：保留期内它是取证材料；等超过 retention 再删；
4. 全量排查：`keytool audit --keystore /app/keys/keystore.json --limit 200`
   确认是否有非预期 rotate/retire/delete/backup/restore 记录（actor 字段可定位到服务身份或 keytool）；
5. 若备份文件也可能泄露：所有从该备份恢复过的环境都视为已泄露，逐一按 1-3 处置。

---

## 8. 审计日志检查

每个生命周期操作（create/import/rotate/retire/delete/backup/restore 及失败）
都会追加到 `keystore-audit.jsonl`（与 keystore 同目录，0600，逐条 fsync）。

```bash
docker exec <token-container> /app/keytool audit --keystore /app/keys/keystore.json --limit 50
```

- 记录形态（与 `audit_events` 行一致）：
  `event_id / event_type(token.jwt_key.*) / occurred_at / trace_id / producer / payload{actor, action, result, kid, related_kid, detail}`；
- **绝不包含任何私钥/PEM/凭证**；怀疑泄露时可用
  `docker exec <token-container> grep -c "BEGIN" /app/keys/keystore-audit.jsonl`，正常应为 0；
- 写入失败会打 `ERROR` 日志并累加指标
  `token_keystore_audit_write_failures_total`（非 0 表示有操作未被审计，需立即排查磁盘）；
- 该账本当前未投递到 audit-service（原因见 spec §2.1：离线 keytool 无 DB/MQ、文件状态无法与
  DB 事务原子）。如需统一检索，按 spec 的 follow-up 接入事件管道。

---

## 9. 风险与注意事项

- **单实例子**：多副本不受支持，见 §1；违规并发会显式报 `Stale`/`LockBusy`，不会静默丢钥。
- **锁是本地文件锁 + 内核锁**（`keystore.json.lock`）：持有者保持文件句柄打开并持有
  内核级互斥锁（Unix `flock` / Windows 独占共享模式），锁文件记录 owner token/pid/时间。
  持有者存活期间锁**绝不会被回收**（不存在按 mtime 误删长时间操作的可能）；持有者退出
  或崩溃后，下一次操作自动接管。只覆盖同一挂载卷上的进程；NFS/多节点共享卷不保证
  `flock`/独占打开语义，需先在目标存储上验证。
- **备份未加密**：依赖密管存储的访问控制；泄露即等于私钥泄露（§7）。
- **恢复会替换整套密钥状态**：用旧备份恢复会丢掉备份之后新建的钥（如已轮换的新钥），
  恢复前快照是唯一回退途径；执行前确认备份时间点。
- **删除不可逆**：仅在过期 + 保留期满后执行，执行前确认有可用备份。
- **旧格式备份不可 restore**：PR #7 裸 keystore 备份会被拒绝，升级流程见 §4.1。
- `keytool` 打印内容只有 kid/状态/时间/checksum；任何时候都不要 `cat keystore.json` 或把 PEM 贴进工单。
