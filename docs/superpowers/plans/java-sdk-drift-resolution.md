# Java SDK 端点漂移处理计划（2026-08）

> **生成时间**：2026-08-18
> **审计基准**：`sdk/java/KNOWN-DRIFT.md` + `scripts/check-java-sdk-drift.js` 实际运行输出
> **审计 commit**：`a9b83ca fix(sdk): Java SDK 12 处端点漂移全部修复`（2026-07-30）
> **当前 git HEAD**：`6d070a2 feat(observability): e2e log traceability drill + runbook + ai-context sync + log-format gate`
> **任务**：核对"12 处 drift"当前真实状态，为每条未修复项制定 A/B/C 处理方案

---

## 1. 审计结论速览

`node scripts/check-java-sdk-drift.js` 当前**真实输出**：

```
✗ Java SDK drift detected: 1 issue(s)
  [digua-notification-sdk] notification-service: 1 paths not wrapped by SDK (sample: /ntf/internal/migrations/state)
EXIT=0
```

| 指标 | 当前 | 备注 |
|------|------|------|
| 硬 drift（exit ≠ 0） | **0** | CI 通过 |
| 信息性 issue（exit 0） | **1** | `/ntf/internal/migrations/state` — 内部端点，合理忽略 |
| git tracked 残留 SDK 模块 | **0** | audit/config/file SDK 已被 a9b83ca 完全删除 |
| 物理目录残留 | **1** | `sdk/java/notification-service-sdk/target/`（gitignored，但未 `git rm`） |

**核心结论**：

- 历史 12 处硬 drift 中 **11 处**已通过"删除 SDK 模块 / 改 SDK 路径"被修复（a9b83ca）
- **0 处**当前活跃的硬 drift
- 唯一活跃 issue 是 informational（合理），按 A/B/C 框架属于"无需处理（C 的特殊形式）"
- 但有 3 处**未清理的尾巴**需要后续 commit 收口

---

## 2. "12 处 drift"逐条核对（基于实际文件 vs KNOWN-DRIFT.md 声明）

### 2.1 来源比对

`a9b83ca^:sdk/java/KNOWN-DRIFT.md`（修复前）的"硬失败（12 项 · SDK 调用的路径在 OpenAPI 中不存在）"表格实际只列了 **11 行**，"12"是历史口径，可能含 1 个脚本/文档遗留项。当前 a9b83ca 的提交 message 也提到"check-java-sdk-drift.js 修复 3 处 bug"，3 个 bug 不算 drift，可能是误加。

按现有 11 行 + 1 个 informational 列出，完整核对如下。

### 2.2 核对矩阵

| # | SDK 模块 | SDK 调用路径（修复前） | OpenAPI 实际路径 | 修复方式 | **当前真实状态** | 证据来源 |
|---|---------|---------------------|------------------|---------|------------------|---------|
| 1 | audit-service-sdk | `/v1/audit/events` | `/audit/operation` (GET query_operation_logs) | 删模块（a9b83ca） | ✅ **已修复** | 模块目录不存在，git 无追踪 |
| 2 | audit-service-sdk | `/v1/audit/logs` | `/audit/login` (GET query_login_logs) | 删模块（a9b83ca） | ✅ **已修复** | 同上 |
| 3 | audit-service-sdk | `/v1/audit/event-types` | 缺失 | 删模块（a9b83ca） | ✅ **已修复** | 同上 |
| 4 | config-service-sdk | `/v1/config/namespaces` | 缺失 | 删模块（a9b83ca） | ✅ **已修复** | 模块目录不存在，git 无追踪 |
| 5 | config-service-sdk | `/v1/config/items/{id}` | 缺失 | 删模块（a9b83ca） | ✅ **已修复** | 同上 |
| 6 | file-service-sdk | `/v1/file/files/presign` | 缺失 | 删模块（a9b83ca） | ✅ **已修复** | 模块目录不存在，git 无追踪 |
| 7 | file-service-sdk | `/v1/file/files/upload` | 缺失 | 删模块（a9b83ca） | ✅ **已修复** | 同上 |
| 8 | file-service-sdk | `/v1/file/files/{id}` | 缺失 | 删模块（a9b83ca） | ✅ **已修复** | 同上 |
| 9 | notification-service-sdk | `/v1/notification/announcements` | `/notification/announcements` | a9b83ca **未处理此条**（commit 只改 2 行） | ✅ **已被 digua-notification-sdk 重新对齐**（无需 a9b83ca 修） | `digua-notification-sdk/src/main/java/io/digua/notificationsdk/NotificationServiceClient.java:66` 调用 `/v1/notification/announcements`，strip `/v1` 后匹配 |
| 10 | notification-service-sdk | `/v1/notification/notifications/unread-count` | `/notification/unread-count` | a9b83ca 改 SDK 路径 | ✅ **已修复（双 SDK 都对齐）** | a9b83ca diff 行 112；digua-notification-sdk 调用 `/v1/notification/unread-count` ✅ |
| 11 | notification-service-sdk | `/v1/notification/notifications/{id}/read` | `/notification/announcements/{id}/read` | a9b83ca 改 SDK 路径 | ✅ **已修复（双 SDK 都对齐）** | a9b83ca diff 行 127；digua-notification-sdk 调用 `/v1/notification/announcements/{id}/read` ✅ |
| 12 | — | `/ntf/internal/migrations/state` | 内部 service-jwt 端点 | KNOWN-DRIFT.md 标记 informational | �️ **信息性提示，非 hard drift** | `services/notification-service/openapi.yaml:161-179`，security: bearer_auth，SDK 不覆盖（合理） |

**汇总**：

- ✅ 已修复：11 / 11 硬 drift
- ℹ️ 合理 informational：1
- 🔧 未清理的尾巴：3 项（详见 §3）

---

## 3. 未清理的尾巴（A/B/C 处理方案）

### 3.1 尾巴 #1：`sdk/java/notification-service-sdk/` 僵尸目录残留

**现象**：

- a9b83ca 删除了 `notification-service-sdk/src/main/java/com/kudi/coder/notificationsdk/NotificationServiceClient.java`（commit 描述是"修正 2 条路径"，但 4 行 diff 实为同文件的 2 行字符串替换）
- a9b83ca 同时从 `sdk/java/pom.xml`（注：那时是 `digua-sdk-parent:0.3.0`）移除了该模块
- 但**未** `git rm -r sdk/java/notification-service-sdk`，物理目录保留 `target/` 构建残留
- 现状：目录不在 git tracking（`git ls-files sdk/java/notification-service-sdk` 返回空），但 `ls` 仍可见子目录

**证据**：

```
$ git ls-files sdk/java/notification-service-sdk
(empty)

$ ls sdk/java/notification-service-sdk/
target/

$ find sdk/java/notification-service-sdk -name "*.java" -o -name "pom.xml"
(no output)
```

**推荐方案 C：清理残留**（commit `chore(sdk): remove leftover notification-service-sdk/target/ build artifacts`）

理由：

- 不是代码漂移（CI 已通过），是文件系统卫生问题
- A（保留并复活）— 不必要，新通知 SDK 已在 `digua-notification-sdk` 中完整覆盖
- B（移到 `archived/`）— 历史已用 git history 记录，无需物理归档
- C（清理）— 简单 `git rm -rf` 或 `rm -rf`（target/ 是 gitignored）

**Commit 计划**：

```
chore(sdk): prune notification-service-sdk/target/ build artifacts
- notification-service-sdk/ 已迁移到 digua-notification-sdk，pom.xml/src 早被 a9b83ca 删除
- 仅剩 target/ 构建残留（gitignored），本地目录干扰视觉与搜索
- 不影响 CI / 编译 / 分发
```

---

### 3.2 尾巴 #2：`sdk/java/README.md` 仍列出已删除模块

**现象**：

- `sdk/java/README.md` 第 11-13 行表格仍列出：
  - `audit-service-sdk`
  - `notification-service-sdk`
  - `file-service-sdk`
  - `config-service-sdk`
- 但 4 个模块**全部已被 a9b83ca 删除**（audit / config / file 是真删；notification-service-sdk 只剩 target/ 残留）
- 当前 SDK parent pom（1.1.0）只注册 7 个 `digua-*` 模块，README 描述与实际构建不一致

**推荐方案 A：更新 README 表格**（commit `docs(sdk): sync sdk/java/README.md module table to 1.1.0`）

理由：

- README 是开发者 onramp 第一接触点，文档/实现漂移会误导新人
- 与 A9b83ca 的"删除模块"配套，本应是同一 commit 的一部分（a9b83ca 漏改了 README）
- B（保留 README）— 接受误导，下游消费者会用错路径
- C（删除 README）— 不可取，README 是 SDK 套件的入口

**Commit 计划**：

```
docs(sdk): sync sdk/java/README.md module table to v1.1.0

删 4 行（audit/config/file/notification-service-sdk），增 3 行（digua-user-*/digua-token-*/digua-notification-sdk）。
保持 0.2.0 changelog 不变（仍是 token-server-client-sdk/user-service-* 的 v0.2.0 历史说明）。
```

---

### 3.3 尾巴 #3：`KNOWN-DRIFT.md` 与 README/pom 三方不一致

**现象**：

- `KNOWN-DRIFT.md` 第 5 行："✅ 全部已处理 — 12 → 0 hard fails"
- `KNOWN-DRIFT.md` 第 11-14 行表格只列 4 个模块（audit/config/file/notification-service-sdk），全标 ✅
- `sdk/java/README.md` 仍把这 4 个列入模块清单
- `sdk/java/pom.xml`（1.1.0）一个都不注册
- 三方文档相互矛盾，新人会困惑"SDK 到底是哪些"

**推荐方案 A：把 KNOWN-DRIFT.md 改为历史档案**（commit `docs(sdk): mark KNOWN-DRIFT.md as historical archive`）

理由：

- "12 → 0"是 2026-07-30 那一刻的事实陈述，到 2026-08-18 已经过去 19 天
- 但 KNOWN-DRIFT.md 文件名暗示"still drifting"，实际已经 0 drift
- B（删除文件）— 失去修复决策记录（a9b83ca 的 commit message 已记录，但 KNOWN-DRIFT 表格更易读）
- C（保留原样）— 继续误导

**Commit 计划**：

```
docs(sdk): convert KNOWN-DRIFT.md to historical archive

文件顶部加 "> ⚠️ 历史档案：本文档记录 2026-07-29 时的 12 处 drift。
当前硬 drift 数 = 0（见 `node scripts/check-java-sdk-drift.js`）。
不要在本文档追加新 drift —— 改在 PR 中处理并直接 fix。"

避免新人按文件名 grep 误以为仍有未处理 drift。
```

---

### 3.4 尾巴 #4：`/ntf/internal/migrations/state` informational issue（来自 digua-notification-sdk）

**现象**：

- 当前 check 报告：`digua-notification-sdk` 包装 3 个路径，OpenAPI 定义 7 个，去掉含路径参数的 4 个，剩 3 个无参路径 → 1 个 missing = `/ntf/internal/migrations/state`
- 该端点鉴权 `bearer_auth: []`，归属 `tags: - internal`，是 Phase 10 G3 的 service-jwt 探针
- SDK 是给终端用户 / 服务间业务用的，不需要也不应该调用内部端点

**推荐方案 C（特殊形式）：脚本白名单 / SDK 注释说明**

理由：

- 内部端点**永远**不应该被终端 SDK 覆盖（违反 least privilege）
- A（删端点）— 不能删，`migrations-state` 是健康监控必需
- B（让 SDK 调用）— SDK 没 service-jwt，401；并且不该有调用入口
- C（标记 informational）— 已经是 informational（exit 0），只需在 `KNOWN-DRIFT.md` 历史档案中说明"白名单内部端点"

**Commit 计划**：

```
docs(sdk): whitelist /ntf/internal/migrations/state in KNOWN-DRIFT.md

加一段：
"## 内部端点白名单（CI informational 不视为 drift）
- /ntf/internal/migrations/state（notification-service, service-jwt）
- /audit/internal/migrations/state（audit-service, service-jwt）
- /file/internal/migrations/state（file-service, service-jwt）
- /ntf/internal/* / /audit/internal/* / /file/internal/* （任何 service-jwt 内部探针）
以上端点不应被终端 SDK 调用；OpenAPI 出现但 SDK 不包视为合理。"
```

但注意 audit/file 当前无 SDK 模块（已删），所以白名单目前只有 ntf 一条生效。

---

## 4. notification-service 的 4 处 drift 端点签名建议

> 用户在任务中明确要求"对 notification-service 的 4 处 drift 给出具体端点签名建议"。
>
> **事实校正**：基于实际文件内容，notification-service 在 KNOWN-DRIFT.md 中只有 **3 条硬 drift**（#9-#11）。第 4 处是 informational 的 `/ntf/internal/migrations/state`。
>
> 下面 4 条全部已**事实上修复**（a9b83ca + digua-notification-sdk 重新对齐），但仍给出端点签名供未来 SDK 生成参考。

### 4.1 #9 — `POST /notification/announcements`（创建公告）

**当前状态**：✅ **已对齐**

- digua-notification-sdk 当前调用：`baseRequest("/v1/notification/announcements").post(...)` —— strip `/v1` 后是 `/notification/announcements`，匹配 openapi.yaml 第 9 行
- 服务侧 openapi.yaml 第 39-55 行已定义

**端点签名（服务侧权威来源：`services/notification-service/openapi.yaml:39-55`）**：

```yaml
/notification/announcements:
  post:
    tags: [announcements]
    summary: Create announcement
    operationId: create
    requestBody:
      required: true
      content:
        application/json:
          schema:
            $ref: '#/components/schemas/CreateAnnouncement'
    responses:
      '201':
        description: 创建成功
        content:
          application/json:
            schema: {}
      '403':
        description: 无权限
```

**SDK 调用形态**（`io.digua.notificationsdk.NotificationServiceClient.createAnnouncement`）：

```java
public Announcement createAnnouncement(AnnouncementRequest req) {
    String body = mapper.writeValueAsString(req);
    Request httpReq = baseRequest("/v1/notification/announcements")
            .post(RequestBody.create(body, JSON))
            .build();
    // ...
}
```

✅ 无需改动。若未来要做增强：建议补 `GET /notification/announcements` 的 SDK 包装（已存在 `listMyAnnouncements` 但参数不同，是 `pinned+page+size` 而 openapi 是 `pinned+active`）。

---

### 4.2 #10 — `GET /notification/unread-count`（未读数）

**当前状态**：✅ **已对齐**

- a9b83ca 把 `com.kudi.coder.notificationsdk.NotificationServiceClient.unreadCount()` 从 `/v1/notification/notifications/unread-count` 改为 `/v1/notification/unread-count`
- digua-notification-sdk 当前一致
- 服务侧 openapi.yaml 第 149-160 行已定义

**端点签名**：

```yaml
/notification/unread-count:
  get:
    tags: [notifications]
    summary: Get unread notification count
    operationId: unread_count
    responses:
      '200':
        description: 获取未读数
        content:
          application/json:
            schema: {}  # ⚠️ 当前 openapi 未声明返回类型，建议补
```

**建议补充 schema**（SDK 返回值强类型化需要）：

```yaml
          schema:
            type: object
            required: [count]
            properties:
              count:
                type: integer
                format: int64
                minimum: 0
```

**SDK 调用形态**（`NotificationServiceClient.unreadCount`）：

```java
public long unreadCount() {
    Request httpReq = baseRequest("/v1/notification/unread-count").get().build();
    // 当前解析为 List<Object> 然后取第一个转 long —— 不健壮
    // 建议改为强类型：
    //   UnreadCountResponse resp = mapper.readValue(respBody, UnreadCountResponse.class);
    //   return resp.count;
}
```

⚠️ **轻微问题**：当前 openapi 的 `schema: {}`（无字段定义）导致 SDK 反序列化时退化成 `List<Object>`。建议给 openapi 补 `UnreadCountResponse { count: long }` schema，然后 SDK 同步改为 DTO。

---

### 4.3 #11 — `POST /notification/announcements/{id}/read`（标记已读）

**当前状态**：✅ **已对齐**

- a9b83ca 把 `markAsRead` 从 `/v1/notification/notifications/{id}/read` 改为 `/v1/notification/announcements/{id}/read`
- digua-notification-sdk 当前一致
- 服务侧 openapi.yaml 第 105-125 行已定义

**端点签名**：

```yaml
/notification/announcements/{id}/read:
  post:
    tags: [notifications]
    summary: Mark announcement as read
    operationId: read_one
    parameters:
      - name: id
        in: path
        required: true
        schema:
          type: integer
          format: int64
          minimum: 0
    responses:
      '200':
        description: 标记已读
        content:
          application/json:
            schema: {}
```

**SDK 调用形态**：

```java
public void markAsRead(long id) {
    Request httpReq = baseRequest("/v1/notification/announcements/" + id + "/read")
            .post(RequestBody.create("", JSON))
            .build();
    // ...
}
```

✅ 无需改动。

---

### 4.4 #12 — `GET /ntf/internal/migrations/state`（service-jwt 内部探针）

**当前状态**：ℹ️ **informational，按设计 SDK 不调用**

- 服务侧 openapi.yaml 第 161-179 行
- 鉴权：`security: [{ bearer_auth: [] }]`，实际要求 service-jwt `aud=notification-service`
- SDK 是给终端用户/服务间业务用的，没 service-jwt 凭据

**端点签名**（仅供参考，SDK 不会调用）：

```yaml
/ntf/internal/migrations/state:
  get:
    tags: [internal]
    summary: Get notification schema migration state
    operationId: handler
    responses:
      '200':
        description: schema state
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/MigrationStateResponse'
      '401':
        description: missing or invalid service-jwt
      '503':
        description: db_unreachable
    security:
      - bearer_auth: []
```

**SDK 处理建议**：无需添加方法。如果需要做 SDK 自检（"我自己健康吗"），应该走 `/health`（公开端点），不应用这个内部端点。

---

## 5. Commit 计划（按依赖顺序）

### 5.1 Commit 依赖图

```
chore(sdk): prune notification-service-sdk/target/ build artifacts  [P0, 1 文件]
              ↓
docs(sdk): sync sdk/java/README.md module table to v1.1.0          [P0, 1 文件]
              ↓
docs(sdk): convert KNOWN-DRIFT.md to historical archive             [P0, 1 文件]
              ↓
docs(sdk): whitelist internal /ntf/audit/file migrations-state      [P1, 1 文件]
```

4 个独立 commit，可任意顺序；以下用 5.2 顺序便于审计追溯。

### 5.2 详细 commit 计划

#### Commit 1 — `chore(sdk): prune notification-service-sdk/target/ build artifacts`

**类型**：`chore`
**范围**：`sdk/`
**影响文件**：
- `sdk/java/notification-service-sdk/target/`（删除，gitignored，但 `rm -rf` 物理清理）

**变更说明**：

- a9b83ca 已删除该模块的 `pom.xml` 与 `src/`，但未物理清理 `target/` 构建产物
- 该目录不在 git tracking（`git ls-files` 为空），但 `ls sdk/java/` 仍能看到
- 不影响 CI / 编译 / 分发

**回滚**：N/A（物理删除 gitignored 目录）

**验证**：

```bash
git status --porcelain            # 期望：空
ls sdk/java/notification-service-sdk/  # 期望：No such file or directory
node scripts/check-java-sdk-drift.js   # 期望：仍 exit 0
```

---

#### Commit 2 — `docs(sdk): sync sdk/java/README.md module table to v1.1.0`

**类型**：`docs`
**范围**：`sdk/java/README.md`
**影响文件**：
- `sdk/java/README.md`（单文件修改，删 4 行 + 增 3 行表格）

**变更说明**：

- 当前 README 表格（行 11-13）仍列：`audit-service-sdk`、`notification-service-sdk`、`file-service-sdk`、`config-service-sdk`
- 4 个模块已被 a9b83ca 删除（audit/config/file 完整删除；notification-service-sdk 仅 target/ 残留）
- 应同步到 `sdk/java/pom.xml`（v1.1.0）的实际 7 个模块：common / digua-user-user-sdk / digua-user-client-sdk / digua-token-user-sdk / digua-token-client-sdk / digua-http-client / digua-notification-sdk

**验证**：

```bash
# 编译时 README 不影响 maven 构建
mvn -f sdk/java/pom.xml validate  # 期望：通过
diff <(grep module sdk/java/pom.xml) <(grep artifactId sdk/java/README.md)  # 期望：无差异
```

---

#### Commit 3 — `docs(sdk): convert KNOWN-DRIFT.md to historical archive`

**类型**：`docs`
**范围**：`sdk/java/KNOWN-DRIFT.md`
**影响文件**：
- `sdk/java/KNOWN-DRIFT.md`（单文件顶部加 4 行警示框）

**变更说明**：

- 文件名暗示"已知 drift"，实际当前 0 硬 drift
- 加顶部警示框，明确这是 2026-07-29 历史快照，不是当前状态
- 引导读者去跑 `node scripts/check-java-sdk-drift.js` 拿实时数据

**验证**：

```bash
# Markdown 渲染检查
grep -A2 "历史档案" sdk/java/KNOWN-DRIFT.md  # 期望：找到
```

---

#### Commit 4 — `docs(sdk): whitelist internal service-jwt probe endpoints`

**类型**：`docs`
**范围**：`sdk/java/KNOWN-DRIFT.md`（追加章节）或新文件 `docs/superpowers/specs/java-sdk-internal-endpoints-whitelist.md`
**影响文件**：1 文件

**变更说明**：

- 显式列出 service-jwt 鉴权的内部端点，告知 SDK 维护者不要把这些路径加进终端 SDK
- 当前活跃白名单：
  - `GET /ntf/internal/migrations/state`（notification-service）
- 潜在白名单（audit-service / file-service 当前无 SDK，未来若有应预先加入）：
  - `GET /audit/internal/migrations/state`（audit-service）
  - `GET /file/internal/migrations/state`（file-service）
- 可选：在 `scripts/check-java-sdk-drift.js` 加 `--internal-whitelist` 参数，让脚本对白名单端点不报告 informational

**验证**：

```bash
node scripts/check-java-sdk-drift.js  # 期望：仍 exit 0，1 个 informational issue 不变
```

---

## 6. 不在 commit 计划中的事项（明确 NOT DO）

- ❌ 不复活 `audit-service-sdk` / `config-service-sdk` / `file-service-sdk` — 服务侧 consumer-only 或路径设计已变更
- ❌ 不改 `digua-notification-sdk` 的 5 个调用方法 — 已对齐 openapi.yaml
- ❌ 不动 `.gitlab-ci.yml` 第 195-207 行的 `java_sdk_drift_check` — `allow_failure: false` 正确，CI 已通过
- ❌ 不补 `UnreadCountResponse` schema — 见 §4.2 备注，是 nice-to-have，不在本计划范围（建议开独立 ticket）
- ❌ 不调整 `notification-service-sdk` 模块复活 — 与 `digua-notification-sdk` 重名冲突，且前者基于 v0.1.0 API

---

## 7. 执行时间线（建议）

| 阶段 | Commit | 预计工作量 |
|------|--------|-----------|
| Day 0 | Commit 1（清理 target/） | 5 分钟 |
| Day 0 | Commit 2（README 同步） | 10 分钟 |
| Day 0 | Commit 3（KNOWN-DRIFT 历史化） | 5 分钟 |
| Day 0 | Commit 4（白名单文档） | 10 分钟 |
| Day 0 | 跑 CI 验证：`node scripts/check-java-sdk-drift.js` + `mvn -f sdk/java/pom.xml validate` | 5 分钟 |

**总计**：35 分钟，4 commit，全部 docs/chore 类型，0 业务代码改动。

---

## 8. 参考依据

| 文档 | 路径 |
|------|------|
| Drift 检查脚本 | `scripts/check-java-sdk-drift.js` |
| Drift 历史档案 | `sdk/java/KNOWN-DRIFT.md` |
| 修复提交 | `a9b83ca fix(sdk): Java SDK 12 处端点漂移全部修复` |
| 通知服务端点 | `services/notification-service/openapi.yaml` |
| 通知服务规范 | `services/notification-service/AGENTS.md` |
| 当前 SDK parent | `sdk/java/pom.xml`（v1.1.0） |
| 当前活跃 SDK | `sdk/java/digua-notification-sdk/src/main/java/io/digua/notificationsdk/NotificationServiceClient.java` |
| CI 配置 | `.gitlab-ci.yml` 行 195-207 |

---

> **本计划由 `Sisyphus`（MiniMax-M3）基于实际 git HEAD 与运行脚本输出生成，未做任何文件修改。**
