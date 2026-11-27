# LMMS Bootstrap 初始化并发控制设计问答

- 日期：2026-11-28
- 状态：设计已确认，待实施
- 范围：WP-BS-01（LMMS 平台 Bootstrap 初始化，包含 LMMS 内部 SDK 与 AI-PM 调用集成）
- 关联：LMMS 首管理员初始化、`LMMS_INIT_PASSWORD`、`/v1/bootstrap/status`、`/v1/bootstrap`

## 背景

LMMS 首次部署时，需要通过 Bootstrap 流程创建第一个用户，并授予平台级和租户级管理权限。该操作具有一次性和高权限特征，必须防止多个请求或多个 IAM 实例同时创建首管理员。

本设计采用**单请求 2 秒延时准入队列 + 数据库事务级原子保护**。不引入 Redis、RabbitMQ 或独立消息队列。

## Q1：为什么首管理员初始化需要并发控制？

**答：**

如果多个请求同时判断系统尚未初始化，可能发生：

- 多个请求同时通过“尚无用户”的检查。
- 多个请求分别创建首用户，产生多个首管理员。
- 用户创建成功，但角色绑定或初始化状态更新失败，留下半初始化数据。
- 多个 IAM 实例各自接受一个请求，绕过单实例内存队列。

因此，Bootstrap 必须同时具备请求准入控制和数据库级最终保护。

## Q2：是否需要引入 Redis 或独立消息队列？

**答：**

不需要。

请求准入队列只在 IAM 进程内实现，用于控制同一实例的请求顺序。跨实例的一致性由关系数据库事务和行锁保证。

不新增 Redis、RabbitMQ、Kafka 或其他消息中间件，也不把 Bootstrap 请求变成后台持久任务。

## Q3：2 秒延时队列如何工作？

**答：**

采用单请求准入机制，等待窗口为 2 秒：

1. Bootstrap 请求到达后，先校验 Bootstrap 是否启用、初始化状态、初始化密码及请求参数。
2. 密码错误或 Bootstrap 已关闭的请求不占用队列。
3. 第一个有效请求取得当前 IAM 实例的队列准入权。
4. 请求进入 2 秒等待窗口。
5. 等待期间，其他请求发现队列已被占用，立即拒绝，不进入队列。
6. 2 秒结束后，第一个请求进入数据库事务并执行初始化。
7. 初始化成功或失败后，释放当前实例的队列准入权。

**核心原则：只处理第一个有效请求；等待窗口内的其他请求全部拒绝，不排队、不自动重试、不在原请求失败后递补执行。**

这里的“队列”是单槽准入器，不是 FIFO 队列，也不保存多个待执行任务。

## Q4：其他请求应该返回什么状态码？

**答：**

| 情况 | HTTP 状态码 | 说明 |
| --- | ---: | --- |
| 初始化密码错误 | 403 | 无权执行初始化 |
| 当前实例已有请求占用准入槽 | 409 | Bootstrap 请求正在等待或执行 |
| 数据库原子竞争失败 | 409 | 其他请求已先完成或取得初始化权 |
| 初始化已经完成 | 404 | Bootstrap 接口永久关闭 |
| 未配置 `LMMS_INIT_PASSWORD` | 404 | Bootstrap 功能未启用 |
| 请求参数不合法 | 400 | 请求内容不符合接口契约 |

队列占用使用 409 Conflict，而不是 403。403 专门表达身份凭据校验失败，便于客户端区分密码错误和并发冲突。

## Q5：仅靠延时队列能否保证并发安全？

**答：**

不能。

每个 IAM 实例都有独立内存。实例 A 和实例 B 可能各自接受一个请求，因此内存队列不能作为全局锁。

必须在数据库中建立唯一的 Bootstrap 状态记录，并在初始化事务中锁定该记录。建议状态至少包括：

- `not_started`：尚未初始化。
- `completed`：初始化已完成。

数据库记录应由迁移预置为唯一的一行，例如固定主键 `id = 1`。不要依赖“用户表当前是否为空”作为唯一并发锁。

事务执行时，先通过条件更新取得初始化权：

```sql
UPDATE iam_bootstrap_state
SET status = 'initializing'
WHERE id = 1
  AND status = 'not_started';
```

只有影响行数为 1 的事务可以继续创建用户。该更新必须与后续用户、租户、角色及最终状态更新处于**同一个数据库事务**中。

其他实例的竞争更新会等待数据库行锁。首个事务提交后，后续更新影响行数为 0；首个事务回滚后，后续请求才可能取得初始化权。

`initializing` 在事务提交前不对其他事务可见。正常情况下，事务最终只会提交 `completed`，或者回滚回 `not_started`，不会留下已提交的中间状态。

该机制必须分别验证 PostgreSQL 和 MySQL 的实际事务行为。

## Q6：首管理员创建是否需要事务？

**答：**

必须使用一个数据库事务。以下操作作为一个整体成功或失败：

1. 取得 Bootstrap 状态记录的初始化权。
2. 创建首租户（如果部署初始化流程需要创建租户）。
3. 创建首管理员用户。
4. 绑定 `tenant_admin` 角色。
5. 绑定 `platform_admin` 角色。
6. 完成必要的角色权限关联。
7. 将 Bootstrap 状态更新为 `completed`。

任何一步失败，整个事务回滚。不得出现用户已创建但角色未绑定、或初始化状态与实际用户数据不一致的情况。

首管理员必须同时拥有 `platform_admin` 和 `tenant_admin`。后续普通租户管理员的角色授予不属于本次 Bootstrap 首管理员流程。

## Q7：第一个请求执行失败怎么办？

**答：**

如果第一个请求在数据库事务中执行失败：

1. 回滚整个事务。
2. Bootstrap 状态记录随事务回滚，恢复为 `not_started`。
3. 释放当前 IAM 实例的内存准入槽。
4. 不恢复已经被拒绝的其他请求。
5. 用户可以重新发起一次 Bootstrap 请求。

这可以避免初始化失败后永久锁死，同时保持“每轮只处理第一个请求”的规则。

如果失败发生在事务提交结果不确定的网络断连场景，服务端不能仅凭连接错误判断事务一定回滚。重新尝试前必须重新读取数据库中的 Bootstrap 状态；如果状态为 `completed`，按初始化成功处理并关闭 Bootstrap。

## Q8：如果 IAM 服务在 2 秒等待期间重启怎么办？

**答：**

内存准入槽会随进程退出而丢失。等待中的请求不持久化，也不自动恢复。

重启后：

- 数据库状态为 `not_started`：允许用户重新发起 Bootstrap。
- 数据库状态为 `completed`：Bootstrap 接口继续返回 404。
- 初始化事务尚未提交时进程退出：数据库回滚未提交事务，状态恢复为 `not_started`。
- 初始化事务已经提交时进程退出：数据库状态保持 `completed`，不得再次初始化。

因此，进程重启不会导致重复创建首管理员。

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
| 请求准入 | IAM 进程内单槽队列 |
| 等待窗口 | 2 秒 |
| 请求策略 | 每个实例只接受第一个有效请求 |
| 后续请求 | 立即返回 409，不排队、不自动重试 |
| 跨实例保护 | 数据库唯一状态行 + 事务内条件更新/行锁 |
| 状态更新 | 初始化权获取、业务数据创建、完成状态在同一事务 |
| 失败恢复 | 事务回滚，释放内存准入槽，允许重新发起 |
| 服务重启 | 不恢复内存队列，以数据库持久状态为准 |
| 初始化成功 | 永久关闭 Bootstrap，接口返回 404 |
| 外部依赖 | 不增加 Redis 或独立消息队列 |
| 内部服务关系 | 由 LMMS 自主定义和初始化，不接受 AI-PM 传入 |
| SDK | 在 LMMS 仓库内升级，提供 BootstrapClient |
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

### 请求队列

- [ ] 第一个有效请求进入 2 秒等待窗口。
- [ ] 同一 IAM 实例在等待窗口内只接受一个请求。
- [ ] 第二个及后续请求立即返回 409，不进入队列。
- [ ] 错误密码请求返回 403，且不占用队列。
- [ ] 首请求结束后准入槽正确释放。

### 数据库并发与事务

- [ ] Bootstrap 状态表有且仅有一条初始化状态记录。
- [ ] 多个 IAM 实例同时收到请求时，最终只有一个事务取得初始化权。
- [ ] 用户、租户、角色绑定和完成状态在同一事务中提交。
- [ ] 任一创建步骤失败时，事务整体回滚。
- [ ] PostgreSQL 和 MySQL 均通过并发、回滚和重启恢复测试。
- [ ] 验证数据库提交结果不确定时的恢复行为。

### 安全与关闭

- [ ] 未配置 `LMMS_INIT_PASSWORD` 时 Bootstrap 接口返回 404。
- [ ] 初始化密码错误时不创建任何用户或服务身份。
- [ ] 首管理员同时拥有 `platform_admin` 和 `tenant_admin`。
- [ ] 初始化成功后 status 和 bootstrap 接口均返回 404。
- [ ] 重启服务不能重新开放 Bootstrap。
- [ ] Bootstrap 密码不被复用为登录密码或服务间凭据。
- [ ] 普通登录页不自动触发 Bootstrap status 请求。

## 最终决策

**采用 2 秒单请求延时准入队列 + 数据库事务级原子并发保护 + 单事务完成首管理员初始化。**

不引入 Redis 或独立消息队列。内存队列只负责同实例请求协调，数据库事务负责跨实例唯一性和数据一致性。
