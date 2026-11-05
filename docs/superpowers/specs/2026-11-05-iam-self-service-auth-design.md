# iam-service 自服务认证设计（忘记密码 / 邀请加入 / 自助资料）

> 日期：2026-11-05
> 状态：待实施
> 下游消费方：`ai-pm/ai-frontend`（经 api-gateway `/gw`）

## 一、前因后果

### 1.1 背景

ai-pm 前端的登录、忘记密码、重置密码、接受邀请四个页面已按 Figma「02 — Authentication」落地，
但 `src/api/auth.ts` 的 `authGateway` 至今写死为 mock：任意非空凭据可登录、忘记密码恒成功、
重置票据只用字符串 `expired` 模拟。个人资料页与安全页同样是 `useDemoForm` 的 600ms 假保存。

后端事实：

- 认证真源在上游 `iam-service`，已有 `login / refresh / logout / password / register / setup-status / sessions`。
- `forgot-password`、`reset-password`、邀请接受、自助资料（`/iam/users/me`）均不存在。
- `CreateUserRequest.password` 必填，管理员建号必须线下告知密码；`ResetPasswordRequest` 是孤儿 DTO
  （只在 openapi.rs 的 schema 列表里，没有路由与 handler）。
- 全仓没有任何邮件/SMS 发送能力（`grep smtp|mailer|lettre` 零命中）。

### 1.2 目标

1. 忘记密码：用户提交邮箱 → 收到一次性重置链接 → 设置新密码。接口恒返回成功，避免账号枚举。
2. 邀请加入：管理员建号时生成一次性邀请票据并发邮件 → 用户点链接设置密码 → 账号激活并保留管理员分配的角色。
3. 自助资料：登录用户可读改 `nickname` 与 `avatar`；邮箱、手机号只读（验证流程不在本轮）。
4. 修改密码：复用既有 `POST /iam/auth/password`，前端真实接入。

### 1.3 非目标

- 开放自助注册（`register` 仍只允许零用户时创建首个管理员）。
- 邮箱/手机号变更与验证、2FA、通行密钥。
- 多租户邀请跨租户迁移。

### 1.4 关键裁决

| 议题 | 裁决 | 理由 |
|---|---|---|
| 认证归属 | iam-service | 身份单源；ai-pm 只消费 SDK，不另建账号体系 |
| 重置投递 | SMTP | 前端页面已预设「查收邮件」；开发用 MailHog，生产用客户 SMTP |
| 邀请形态 | 一次性票据 | 避免线下传密码；与「邀请制」语义一致 |
| 资料字段 | nickname + avatar | email/phone 变更需要验证链路，本轮不做 |
| 令牌存储 | 只存 SHA-256 哈希 | 数据库泄漏不能直接换密码/激活账号 |

## 二、数据模型

新增迁移 `V010__iam_self_service.sql`（MySQL + PostgreSQL 各一份，forward-only）。

```sql
CREATE TABLE iam_one_time_tokens (
    id          VARCHAR(64) PRIMARY KEY,
    tenant_id   BIGINT      NOT NULL,
    user_id     VARCHAR(64) NOT NULL,
    kind        VARCHAR(32) NOT NULL,   -- password_reset | invitation
    token_hash  VARCHAR(64) NOT NULL,
    expires_at  TIMESTAMP   NOT NULL,
    consumed_at TIMESTAMP   NULL,
    created_at  TIMESTAMP   NOT NULL,
    CONSTRAINT uq_iam_one_time_token_hash UNIQUE (token_hash)
);
CREATE INDEX idx_iam_one_time_tokens_user ON iam_one_time_tokens (user_id, kind);
```

约束与语义：

- 一个用户可有多条历史令牌；消费后写 `consumed_at`，不再可用。
- 同一 `(user_id, kind)` 新建时先作废该用户未消费的旧令牌（`consumed_at = NOW()`），避免多链接并存。
- `invitation` 默认 7 天过期；`password_reset` 默认 30 分钟；由配置控制。
- 令牌明文只在邮件里出现一次，落库仅 SHA-256。

不改 `iam_users` 列：邀请用户先写入随机 bcrypt 口令哈希，登录必然验证失败，激活时覆盖。

## 三、接口契约

新增公开路由（网关 `public = true`）：

| 方法 | 路径 | 请求 | 响应 |
|---|---|---|---|
| POST | `/v1/iam/auth/forgot-password` | `{email}` | `204`（恒成功，账号不存在也返回 204） |
| POST | `/v1/iam/auth/reset-password` | `{token, new_password}` | `204`；令牌无效/过期/已用 → `400 invalid_or_expired_token` |
| POST | `/v1/iam/auth/invitations/accept` | `{token, password, nickname?}` | `200 LoginResponse`（激活后直接登录） |

新增鉴权路由（`/v1/iam/users/{rest}` 已受网关保护）：

| 方法 | 路径 | 请求 | 响应 |
|---|---|---|---|
| GET | `/v1/iam/users/me` | — | `UserResponse` |
| PUT | `/v1/iam/users/me` | `{nickname?, avatar?}` | `UserResponse` |

管理员建号改造（`POST /v1/iam/users`）：

- `CreateUserRequest.password` 改为可选。
- 新增 `send_invitation: bool`（默认 `true`）。
- `password` 缺失且 `send_invitation = true` → 建号 + 邀请票 + 发邮件，响应仍是 `UserResponse`。
- `password` 缺失且 `send_invitation = false` → `400 password_required`。
- 既有「首用户注册」路径不受影响（`register` 内部仍要求密码）。

兼容性说明：本仓处于开发阶段、无存量数据，按仓库 AGENTS 规则直接改契约，不做双写/过渡层。

## 四、邮件投递

新增 `services/iam-service/src/mail.rs`，使用 `lettre`（rustls，不用 native-tls）。

配置（env > yaml）：

| 变量 | yaml 键 | 默认 | 说明 |
|---|---|---|---|
| `IAM_SMTP_HOST` | `smtp.host` | 空 | 空则邮件功能关闭 |
| `IAM_SMTP_PORT` | `smtp.port` | 1025 | 开发 MailHog |
| `IAM_SMTP_USERNAME` | `smtp.username` | 空 | 空表示无认证（MailHog） |
| `IAM_SMTP_PASSWORD` | `smtp.password` | 空 | |
| `IAM_SMTP_FROM` | `smtp.from` | `no-reply@ai-pm.local` | |
| `IAM_SMTP_STARTTLS` | `smtp.starttls` | false | 生产 SMTP 置 true |
| `IAM_PUBLIC_BASE_URL` | `public.base_url` | `http://127.0.0.1:8848` | 拼接前端链接 |

链接：

- 重置：`{base}/auth/reset-password?token={token}`
- 邀请：`{base}/auth/invitation?token={token}`

失败语义：

- SMTP 未配置 → 对应端点返回 `503 mail_not_configured`（不伪装成功，避免用户以为已发信）。
- SMTP 发送失败 → `502 mail_delivery_failed`，令牌作废（不留下无法送达的有效票据）。
- 账号不存在/已禁用 → 仍返回 204，不发送、不留痕。

## 五、安全约束

1. 令牌 32 字节随机（nanoid 64 字符），只存 SHA-256。
2. 消费使用条件更新：`UPDATE ... SET consumed_at = NOW() WHERE token_hash = ? AND consumed_at IS NULL AND expires_at > NOW()`，受影响行数为 0 即拒绝（防并发重放）。
3. 重置成功后撤销该用户全部 refresh token，强制其他会话重新登录。
4. 邀请接受只允许 `status = 'active'` 且尚未设置过密码的账号？——不引入新列，改为：邀请票据只能消费一次，
   且接受时覆盖口令并写 `email_verified = true`（邀请邮件本身即邮箱所有权证明）。
5. 自助资料只允许改白名单字段，`status / roles / tenant_id` 不可自改。
6. `/iam/users/me` 从 `Principal.user_id` 取身份，禁止从 body/path 指定 user_id。

## 六、实施顺序

| 阶段 | 内容 | 验收 |
|---|---|---|
| S1 | 迁移 V010（双库）+ 模型 | 迁移在 MySQL/PG 实际执行；重复执行幂等 |
| S2 | DTO / service / routes（先不接邮件，用日志 backend） | 单测覆盖：重置成功、令牌过期、令牌重放、邀请过期、资料白名单 |
| S3 | lettre SMTP + 配置 + MailHog compose | 本地 MailHog 收到重置/邀请邮件 |
| S4 | 网关 public 路由 + openapi 重新生成 + 契约门禁 | `dump-openapi-specs.js`、`check-spec.js`、`contracts-gen.js --check` 全过 |
| S5 | SDK `iam-services` 增补方法 | `pnpm packages:check` / 单测通过 |

## 七、风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| 引入 lettre 增加编译依赖 | 构建变慢/依赖冲突 | 只用 rustls 特性；先验证 `cargo check -p iam-service` |
| Windows 原生不可编译 | 本机无法执行 iam 测试 | 按仓库规则走 WSL2 backend 或 docker 容器测试 |
| 邮件在开发环境不可用 | 前端无法端到端 | MailHog 容器随栈启动，默认 SMTP 指向它 |
| 邀请用户占位口令 | 登录接口对占位 hash 的 verify 行为 | 占位值使用合法 bcrypt hash，verify 返回 `Ok(false)` |
| 账号枚举 | 通过 forgot-password 探测邮箱 | 恒 204；发送失败只在服务端日志区分 |

## 八、验收清单

- [ ] 迁移 V010 在 MySQL 与 PostgreSQL 均成功，`iam_one_time_tokens` 存在且唯一索引生效。
- [ ] `forgot-password` 对存在/不存在邮箱均返回 204；存在时 MailHog 收到带 `token` 的链接。
- [ ] `reset-password` 用有效令牌可改密并撤销旧 refresh token；重放同一令牌返回 400。
- [ ] `invitations/accept` 可用邀请令牌设置密码并直接返回登录态；过期令牌返回 400。
- [ ] 管理员建号缺省密码时发邀请邮件；显式传密码时不发。
- [ ] `GET/PUT /iam/users/me` 只能读写本人，`nickname` 与 `avatar` 生效，`email/phone` 不可改。
- [ ] 既有 login / refresh / register / setup-status / password / sessions 全部回归通过。
- [ ] OpenAPI 无漂移，契约门禁通过。