# LMMS Bootstrap 初始化并发控制设计问答

- 日期：2026-11-28
- 状态：设计已确认，待实施
- 范围：WP-BS-01（LMMS 平台 Bootstrap 初始化，包含 LMMS 内部 SDK 与 AI-PM 调用集成）
- 关联：LMMS 首管理员初始化、`LMMS_INIT_PASSWORD`、`/v1/bootstrap/status`、`/v1/bootstrap`

## 背景

LMMS 首次部署时，需要通过 Bootstrap 流程创建第一个用户，并授予平台级和租户级管理权限。该操作具有一次性和高权限特征，必须防止多个请求或多个 IAM 实例同时创建首管理员。

本设计采用**数据库请求记录 + 2 秒自增 ID 仲裁 + 数据库唯一执行权 + JSON Step 状态持久化 + 人工触发恢复**。不引入 Redis、RabbitMQ 或独立消息队列。

## Q1：为什么首管理员初始化需要并发控制？

**答：**

如果多个请求同时判断系统尚未初始化，可能发生：

- 多个请求同时通过“尚无用户”的检查。
- 多个请求分别创建首用户，产生多个首管理员。
- 用户创建成功，但角色绑定或初始化状态更新失败，留下半初始化数据。
- 多个 IAM 实例同时接受请求，若没有数据库级仲裁，可能由不同实例分别执行。

因此，Bootstrap 必须同时具备请求准入控制和数据库级最终保护。

## Q2：是否需要引入 Redis 或独立消息队列？

**答：**

不需要。Bootstrap 的请求收集、唯一执行权和步骤进度均由 LMMS 关系数据库持久化协调，不引入 Redis、RabbitMQ、Kafka 或独立消息队列。

请求记录表采用 `iam_bootstrap_init_requests`。它既保存首次并发请求的仲裁记录，也保存最终胜出初始化流程的步骤进度。

## Q3：多 IAM 实例如何通过 2 秒窗口选出唯一请求？

**答：**

不使用 IAM 进程内存队列。所有 IAM 实例都使用同一数据库表进行请求仲裁。

建议记录字段：

| 字段 | 用途 |
| --- | --- |
| `id` | 数据库自增主键，作为本轮请求的仲裁顺序 |
| `status` | `collecting`、`selected`、`running`、`failed`、`completed`、`rejected` |
| `created_at` | 请求记录创建时间 |
| `started_at` | 最近一次执行开始时间 |
| `finished_at` | 全部步骤完成时间 |
| `steps` | JSON，保存每个初始化步骤的状态和非敏感元数据 |
| `execution_owner` | 当前执行 IAM 实例标识 |
| `execution_epoch` | 每次成功取得执行权时递增的 fencing token |
| `lease_expires_at` | 当前执行权租约到期时间 |
| `last_error` | 最近一次失败的错误代码或安全摘要 |
| `updated_at` | 最近更新时间 |

首次初始化的一轮请求按以下方式工作：

1. 每个请求先检查 Bootstrap 是否已完成，以及是否存在正在收集或执行的初始化轮次。
2. 已完成返回 404；已有有效执行权或收集窗口已关闭则返回 409。
3. 当系统尚无初始化记录时，请求插入一条 `collecting` 记录。应用层先检查不构成并发保证；多个 IAM 实例同时检查为空时，允许产生多条记录。
4. 每个成功插入的请求等待 2 秒。
5. 2 秒后，按本轮窗口查询记录，选择 `id` 最小的一条作为唯一候选。候选选择和状态变更必须通过数据库原子条件更新完成。
6. 候选请求取得执行权后，将其状态更新为 `selected/running`；本轮其余记录标记为 `rejected`，不执行初始化。
7. 其他请求即使等待结束，也必须重新读取数据库状态；未取得执行权的请求返回 409。

仲裁顺序是数据库分配的自增 `id` 顺序，不承诺严格等同于网络层请求到达时间。2 秒是首次请求收集窗口；它不是每个 IAM 实例各自独立的本地队列。

为了避免后到请求误加入下一轮，收集窗口必须以本轮最早记录的 `created_at` 为基准，并在窗口关闭后通过数据库状态阻止继续加入。数据库事务和条件更新负责处理窗口边界的竞争。

## Q4：其他请求应该返回什么状态码？

**答：**

| 情况 | HTTP 状态码 | 说明 |
| --- | ---: | --- |
| 初始化密码错误 | 403 | 无权执行初始化，不创建请求记录 |
| 当前已有有效执行权 | 409 | 另一个 IAM 实例正在执行 |
| 首轮收集窗口内的非胜出请求 | 409 | 该请求未取得执行权 |
| Bootstrap 已完成 | 404 | Bootstrap 接口永久关闭 |
| 未配置 `LMMS_INIT_PASSWORD` | 404 | Bootstrap 功能未启用 |
| 请求参数不合法 | 400 | 请求内容不符合接口契约 |
| 步骤执行失败 | 适当的 5xx | 记录失败步骤，等待用户再次触发恢复 |

## Q5：数据库如何保证只有一个 IAM 实例执行？

**答：**

请求仲裁和执行权是两个不同机制：

- **请求仲裁**：2 秒窗口结束后，按自增 `id` 选出本轮唯一候选。
- **执行权**：通过数据库原子条件更新取得全局唯一执行权，防止两个 IAM 实例同时执行同一个 Bootstrap 流程。

执行权至少包含 `execution_owner`、`execution_epoch` 和 `lease_expires_at`。取得执行权时必须在数据库事务中检查状态并递增 `execution_epoch`。所有步骤状态写入都必须带上当前 epoch 条件，旧执行者不能在执行权被重新授予后继续写入。

示意：

```sql
UPDATE iam_bootstrap_init_requests
SET status = 'running',
    execution_owner = :instance_id,
    execution_epoch = execution_epoch + 1,
    lease_expires_at = :lease_deadline,
    started_at = CURRENT_TIMESTAMP
WHERE id = :selected_request_id
  AND status IN ('selected', 'failed', 'running')
  AND (lease_expires_at IS NULL OR lease_expires_at < CURRENT_TIMESTAMP);
```

实际 SQL 必须按 PostgreSQL 和 MySQL 方言实现，并检查受影响行数。只有影响行数为 1 的 IAM 实例取得执行权。执行期间按需要续租；每次续租和步骤状态更新都校验 owner/epoch。

进程崩溃后，租约到期前其他请求仍返回 409。租约到期后也不会后台自动恢复；只有用户再次访问同一个 Bootstrap URL，才允许新请求取得执行权并恢复。此时应复用首轮胜出的原记录（最小 id），不能新建一条独立 Bootstrap 流程。若旧实例仍在运行，epoch fencing 可阻止它继续提交状态更新。

## Q6：首管理员与后续初始化步骤如何保证一致？

**答：**

Bootstrap 是跨 IAM 与 Token Service 的多步骤流程，不能声称由一个数据库事务覆盖所有服务。采用持久化步骤状态和幂等恢复协议。

每个 Step 统一执行：

1. **Check**：向该对象的权威服务/数据库检查目标对象是否存在。
2. **Insert**：不存在时创建；存在时核对关键属性，不一致则报告冲突，不覆盖既有数据。
3. **Persist**：成功确认后，立即将该 Step 标记为 `completed`，并以当前 `execution_epoch` 条件更新持久化记录。

数据库唯一约束是防止重复创建的最终保障。不能把“先 SELECT 检查不存在，再普通 INSERT”当成并发安全保证；应使用唯一约束配合原子插入、`ON CONFLICT`、等价 upsert 或捕获唯一冲突后重新 Check 的方式。

建议的初始步骤清单：

| Step | 初始化内容 | 权威检查/创建方 |
| --- | --- | --- |
| S01 | 首个租户（若部署流程需要） | IAM |
| S02 | 首管理员用户 | IAM |
| S03 | 首管理员 `platform_admin` 角色绑定 | IAM |
| S04 | 首管理员 `tenant_admin` 角色绑定 | IAM |
| S05 | LMMS 内部服务身份及所需凭据 | LMMS 内部初始化流程 / Token Service |
| S06 | `iam-service → token-service` Grant | Token Service |
| S07 | `notification-service → iam-service` Grant | Token Service |

具体步骤必须根据当前 LMMS 服务拓扑和现有 Token Service 模型确认；新增内部服务时由 LMMS 更新自己的步骤定义，不要求 AI-PM 提交服务清单。

IAM 不应直接读取或修改 Token Service 的私有数据库。跨服务 Step 通过 Token Service 提供的内部接口或 LMMS 内部 provisioning 能力执行，并携带稳定的幂等键（例如 Bootstrap run ID + step key）。Token Service 在自己的数据库事务中完成 Check、唯一约束保护下的创建及结果确认。

首管理员用户、租户、角色绑定等 IAM 本地步骤可以各自使用本地事务；跨服务步骤各自在所属服务内保证原子性。整个 Bootstrap 只有在全部步骤完成后才标记为 `completed`。

## Q7：某个 Step 失败或结果未知怎么办？

**答：**

失败时：

1. 将当前 Step 标记为 `failed`，保存安全的错误代码/摘要和尝试次数。
2. 将 Bootstrap 总状态标记为 `failed`，释放或等待执行租约到期。
3. 保留已经完成的 Step，不回滚其他服务已经成功提交的结果。
4. 不自动选择本轮被拒绝的请求递补执行。
5. 用户必须再次访问同一个 Bootstrap URL，显式触发恢复。恢复请求不再创建新的首轮候选记录，而是定位首轮胜出的最小 id 记录，并在执行租约允许时重新取得该记录的执行权。首轮被标记为 `rejected` 的其他记录始终不递补执行。

恢复时读取同一胜出记录中的 `steps`：

- `completed`：跳过。
- `pending`：执行。
- `failed` 或上次遗留的 `running`：重新执行 Check；若对象已存在且属性符合预期，将其标记为 completed；不存在则创建。
- 对象存在但属性不一致：停止并报告冲突，不覆盖数据。

如果首管理员创建 Step 尚未完成，用户再次提交初始化请求时必须重新提供该 Step 所需的管理员凭据；如果首管理员 Step 已完成，则恢复流程不再需要管理员密码。管理员密码和 Bootstrap Password 均不得写入请求记录或 `steps` JSON。

如果外部服务实际执行成功，但 IAM 在保存 Step 完成状态前崩溃，下一次人工触发会再次调用幂等 Check/Create。服务端唯一约束和幂等键确保不会重复创建。

服务凭据必须由 LMMS 内部安全生成和管理。步骤 JSON 不得保存明文 Token、Bootstrap Password 或其他秘密。凭据生成/保存必须可恢复：若响应丢失，使用稳定身份标识和幂等 provisioning 读取/确认既有凭据状态，不得因此创建第二个身份或向 AI-PM 暴露内部凭据。

## Q8：服务器重启后是否自动恢复？

**答：**

不自动恢复。

- 服务启动时只读取状态，不扫描失败任务并执行。
- 不启动后台恢复 worker，不自动重放任何 Step。
- 只有用户再次访问同一个 Bootstrap URL，才触发执行权申请和步骤恢复。
- 如果旧执行租约尚未到期，返回 409；租约到期后，用户再次触发才可取得新 epoch 并恢复。
- 已完成的 Step 通过 Check 跳过，未完成的 Step 重新执行。
- 全部 Step 完成后才把总状态更新为 `completed`，随后 Bootstrap status 和初始化接口返回 404。

如果 IAM 在某个服务已成功创建对象、但尚未更新 Step 状态时重启，恢复请求会重新 Check 该对象并将 Step 补记为 completed。

## Q9：初始化成功后是否允许再次 Bootstrap？

**答：**

不允许。

初始化成功后，Bootstrap 接口永久关闭：

- `GET /v1/bootstrap/status` 返回 HTTP 404。
- `POST /v1/bootstrap` 返回 HTTP 404。
- 服务重启后仍然返回 HTTP 404。
- 未配置 `LMMS_INIT_PASSWORD` 时，上述接口同样返回 HTTP 404。

不能通过重启服务、修改环境变量或再次提交请求重新开放 Bootstrap。

普通登录页不主动调用 Bootstrap status。只有用户显式访问 AI-PM 初始化页面 `/init` 时，前端才请求 Bootstrap status。

## Q10：Bootstrap 密码和服务身份凭据是否共用？

**答：**

不共用。

- `LMMS_INIT_PASSWORD` 只用于保护 Bootstrap 初始化接口。
- Bootstrap 密码不能用于用户登录。
- Bootstrap 密码不能用于服务间认证。
- LMMS 内部服务身份凭据由 LMMS 自己创建和管理，不属于 AI-PM 的业务初始化参数。
- 不向 AI-PM 返回 IAM、Token Service、Notification Service 等 LMMS 内部服务凭据。

## Q11：AI-PM 与 LMMS 如何分工？

**答：**

Bootstrap 是由 AI-PM 触发、由 LMMS 负责执行的平台初始化操作。

**AI-PM 是触发方，不是 LMMS 内部服务拓扑和授权关系的定义方。**

AI-PM 负责：

- 提供显式访问的初始化页面 `/init`。
- 收集部署人员输入的 Bootstrap Password 和首管理员信息。
- 由 AI-PM 后端调用 LMMS 仓库内的 SDK `BootstrapClient`。
- 将初始化结果展示给部署人员。
- 不直接从浏览器访问 LMMS Bootstrap API。
- 不定义 LMMS 内部服务身份、audience、Grant 或授权关系。
- 不持有或持久化 LMMS 内部服务凭据。

LMMS 负责：

- 定义 Bootstrap 请求和响应契约。
- 验证 Bootstrap Password、控制初始化状态和并发。
- 创建首管理员并授予 `platform_admin` 与 `tenant_admin`。
- 创建和管理 LMMS 自身的内部服务身份。
- 按 LMMS 内部架构建立服务间授权关系。
- 管理服务凭据、JWT Grant 和初始化完成状态。

AI-PM 发起 Bootstrap 只代表触发初始化时机，不代表 AI-PM 有权决定 LMMS 内部创建哪些身份或授权关系。

## Q12：LMMS 内部服务身份和授权关系由谁定义？

**答：**

完全由 LMMS 自己定义、创建、维护和演进，属于 LMMS 平台内部架构资产。

本次 Bootstrap 应依据 LMMS 当前内置服务拓扑建立既有关系，包括：

- `iam-service → token-service`
- `notification-service → iam-service`

箭头表示服务 JWT 的 owner → audience 方向。具体服务身份、Grant 的 scope、system-managed 属性及凭据生成方式，必须复用 LMMS Token Service 现有模型和策略。

上述关系是 LMMS 内部关系，不包含 AI-PM，也不由 AI-PM 的请求 Body 传入。未来 LMMS 增加、删除或调整内部服务时，只需修改 LMMS 自己的内部初始化定义，不要求 AI-PM 同步理解内部拓扑。

Bootstrap 的实现还必须协调现有 `SERVICE_BOOTSTRAP_TOKENS` / `SERVICE_BOOTSTRAP_OWNERS` 启动初始化机制，避免产生重复身份、冲突凭据或绕过现有 Grant 管理规则。

## Q13：LMMS SDK 如何支持 Bootstrap？

**答：**

SDK 位于 LMMS 仓库内部，与 LMMS API 同仓库演进，不创建独立 SDK 仓库。

LMMS 应在现有 SDK 中提供独立的 `BootstrapClient` 能力，至少支持：

- 查询 Bootstrap 状态。
- 提交 Bootstrap 初始化请求。
- 在 Bootstrap 请求中专用地传递 `X-LMMS-Init-Password`。
- 解析初始化成功、密码错误、并发冲突、接口关闭等响应。
- 提供超时、网络错误和服务端错误处理。
- 提供 Rust SDK 与 Java SDK 的一致接口语义及测试（以仓库现有 SDK 支持范围为准）。

Bootstrap Password 只能由 BootstrapClient 用于 Bootstrap API，不得进入普通 API 默认 Header、登录 Token、Service JWT、日志或持久化配置。SDK 不应把内部 Grant 管理能力暴露为 Bootstrap 请求参数。

## Q14：AI-PM 是否直接访问 LMMS Bootstrap API？

**答：**

不直接访问。

AI-PM 浏览器只与 AI-PM 后端通信。AI-PM 后端使用 LMMS 仓库提供的 SDK `BootstrapClient` 调用 LMMS Bootstrap API。

这样可以避免浏览器直接依赖 LMMS 内部服务地址和 API 细节，也避免把平台内部集成逻辑暴露到前端。

AI-PM 后端必须：

- 不记录 Bootstrap Password 或完整请求 Header。
- 不将 Bootstrap Password 写入数据库、缓存或持久化日志。
- 不将密码写入浏览器持久化存储。
- 仅在本次初始化请求生命周期内使用密码。
- 在正式部署环境中通过 HTTPS 调用 LMMS。

## Q15：最终采用什么方案？

**答：**

| 机制 | 决策 |
| --- | --- |
| 请求记录 | 数据库表 `iam_bootstrap_init_requests` |
| 首轮准入 | 多实例共享数据库记录，允许并发插入候选记录 |
| 等待窗口 | 2 秒，以本轮最早记录为基准 |
| 胜出规则 | 窗口结束后选择自增 `id` 最小的记录 |
| 其他请求 | 标记 rejected / 返回 409，不递补执行 |
| 执行权 | 数据库原子获取 + owner/epoch fencing + 租约 |
| 步骤进度 | 胜出记录的 `steps` JSON 持久化 |
| Step 语义 | Check → 不存在则 Insert → 验证 → 持久化 completed |
| 幂等保障 | 权威服务检查 + 数据库唯一约束 + 幂等 provisioning |
| 跨服务一致性 | 分步骤可恢复流程，不假设跨服务共享事务 |
| 失败恢复 | 保留已完成步骤；用户重新访问同一 URL 后继续 |
| 服务重启 | 不自动执行、不启动后台恢复任务 |
| 初始化成功 | 所有 Step 完成后标记 completed，接口永久返回 404 |
| 外部依赖 | 不增加 Redis 或独立消息队列 |
| 内部服务关系 | 由 LMMS 自主定义和初始化，不接受 AI-PM 传入 |
| SDK | 在 LMMS 仓库内提供 BootstrapClient |
| 调用路径 | AI-PM 后端 → LMMS SDK BootstrapClient → LMMS Bootstrap API |
| 浏览器访问 | 浏览器只访问 AI-PM，不直接访问 LMMS Bootstrap API |

## 跨项目实施验收要求

### LMMS 平台与内部服务

- [ ] Bootstrap API 契约由 LMMS 定义，AI-PM 请求不包含 owner、audience、Grant 或内部服务清单。
- [ ] Bootstrap 成功创建首管理员，并授予 `platform_admin` 和 `tenant_admin`。
- [ ] LMMS 自主创建或确认内部服务身份。
- [ ] LMMS 建立 `iam-service → token-service` 内部授权关系。
- [ ] LMMS 建立 `notification-service → iam-service` 内部授权关系。
- [ ] 内部服务关系复用 Token Service 现有身份及 system-managed Grant 模型。
- [ ] 不向 AI-PM 返回 LMMS 内部服务凭据。
- [ ] 与现有 `SERVICE_BOOTSTRAP_TOKENS` / `SERVICE_BOOTSTRAP_OWNERS` 启动机制兼容，不产生重复或冲突身份。

### LMMS SDK 与 AI-PM 集成

- [ ] LMMS 仓库内 SDK 提供 `BootstrapClient`。
- [ ] Rust SDK、Java SDK（按当前 SDK 支持范围）提供一致的 Bootstrap API 语义。
- [ ] Bootstrap Password 只用于 BootstrapClient 请求，不进入普通 API 默认认证 Header。
- [ ] AI-PM 后端通过 SDK 调用 LMMS Bootstrap API。
- [ ] AI-PM 浏览器不直接访问 LMMS Bootstrap API。
- [ ] AI-PM 不定义或提交 LMMS 内部服务授权关系。
- [ ] AI-PM 不持久化 Bootstrap Password，不记录密码或敏感 Header。

## 实施验收要求

### 数据库请求仲裁与执行权

- [ ] 多 IAM 实例同时首次请求时，允许产生多条候选记录。
- [ ] 2 秒窗口结束后，按自增 ID 选择唯一胜出记录。
- [ ] 本轮非胜出记录不会执行，也不会在胜出记录失败后自动递补。
- [ ] 请求窗口关闭后，后续请求不能加入已关闭的首轮。
- [ ] 多个 IAM 实例同时尝试取得执行权时，数据库原子更新只允许一个成功。
- [ ] 执行租约、owner 和 epoch fencing 可阻止旧执行者继续写状态。
- [ ] PostgreSQL 和 MySQL 均通过并发、租约过期和 fencing 测试。

### Step 状态与可恢复执行

- [ ] 胜出记录持久化完整步骤清单和每个 Step 的状态。
- [ ] 每个 Step 执行前检查权威数据；不存在才创建。
- [ ] 已存在对象必须核对关键属性，不一致时停止而不是覆盖。
- [ ] 数据库唯一约束兜底，不能依赖应用层 SELECT 后 INSERT。
- [ ] 每个 Step 成功后立即持久化 completed。
- [ ] IAM 本地事务和 Token Service 本地事务分别保证各自数据一致性，不假设跨服务单事务。
- [ ] 外部 provisioning 支持幂等键，处理“远端成功但本地步骤状态未更新”的不确定结果。
- [ ] 初始化任意 Step 失败后保留已完成步骤，等待用户显式重新触发。
- [ ] 服务器重启不自动恢复、不启动后台任务。
- [ ] 用户再次访问同一个 Bootstrap URL 后，只执行未完成步骤。
- [ ] 只有所有 Step 都完成，Bootstrap 总状态才变为 completed。

### 安全与关闭

- [ ] 未配置 `LMMS_INIT_PASSWORD` 时 Bootstrap 接口返回 404。
- [ ] 初始化密码错误时不创建任何用户或服务身份。
- [ ] 首管理员同时拥有 `platform_admin` 和 `tenant_admin`。
- [ ] 初始化成功后 status 和 bootstrap 接口均返回 404。
- [ ] 重启服务不能重新开放 Bootstrap。
- [ ] Bootstrap 密码不被复用为登录密码或服务间凭据。
- [ ] Step JSON、错误日志和普通 API 日志不包含明文密码或服务 Token。
- [ ] 普通登录页不自动触发 Bootstrap status 请求。

## 最终决策

**采用数据库请求记录 + 2 秒自增 ID 仲裁 + 数据库唯一执行权 + JSON Step 状态持久化 + 人工触发恢复。**

不引入 Redis 或独立消息队列。跨服务初始化按步骤分别保证本地事务和幂等性；全部步骤完成前不得宣告 Bootstrap completed。服务器重启不自动执行，必须由用户再次访问同一个 Bootstrap URL 触发恢复。
