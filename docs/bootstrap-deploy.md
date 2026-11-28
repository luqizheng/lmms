# LMMS Bootstrap 部署契约（内部服务身份与初始化顺序）

> 适用范围：首次部署 LMMS（空数据库）并执行平台 Bootstrap 初始化。
> 设计基线：`docs/superpowers/specs/2026-11-28-bootstrap-concurrency-control-design.md`（PR #25）。
> 本文档不包含任何真实 Token、密码或密钥。

## 1. 职责边界（权威责任方）

| 资产 | 权威创建方 | Bootstrap 中的角色 |
| --- | --- | --- |
| 内部服务身份（`token_bootstrap_tokens`，owner + bootstrap token） | **token-service 启动 seed**（`SERVICE_BOOTSTRAP_TOKENS` / `SERVICE_BOOTSTRAP_OWNERS` env CSV，或留空走内置 dev 默认值） | **不创建、不覆盖**。S05 仅 readiness 校验 |
| 系统授权关系（`token_service_jwt_grants`，code-owned 清单） | **token-service 启动 seed**（`ensure_system_grants`）+ `/internal/bootstrap/ensure-grants` 幂等补插 | S06/S07 幂等 ensure（缺失才插，`system_managed = TRUE`） |
| 首管理员（用户 + platform_admin + tenant_admin） | **iam-service Bootstrap 执行**（S02-S04） | 创建 + 数据库级唯一认领（`admin_user_id`） |

内部授权关系的单一真相源是 token-service 源码中的 `SYSTEM_GRANTS` 常量
（`services/token-service/src/models/service_jwt_grant.rs`）；请求方（含
AI-PM）不得传入任何拓扑。

## 2. 首次部署前必须准备的环境变量

token-service：

| 变量 | 说明 |
| --- | --- |
| `SERVICE_BOOTSTRAP_TOKENS` | 服务 bootstrap token CSV（顺序与 OWNERS 一一对应）。**必须包含 `iam-service` 与 `notification-service` 两项**；缺失 iam-service 时 token-service 拒绝启动（fail-closed）。留空时使用内置 dev 默认值，**仅限开发环境** |
| `SERVICE_BOOTSTRAP_OWNERS` | owner 名 CSV，与 TOKENS 等长；长度不等 → 拒绝启动（fail-closed） |

iam-service：

| 变量 | 说明 |
| --- | --- |
| `SERVICE_BOOTSTRAP_TOKEN` | iam-service 自己的 bootstrap token（与 token-service 的 CSV 中 `iam-service` 项相同值） |
| `LMMS_INIT_PASSWORD` | Bootstrap API 专用密码。**未配置 → Bootstrap API 返回 404（功能关闭）**；Bootstrap 完成后同样 404。只用于 Bootstrap API，不得用于登录或服务间认证 |

notification-service：

| 变量 | 说明 |
| --- | --- |
| `SERVICE_BOOTSTRAP_TOKEN` | notification-service 自己的 bootstrap token（与 CSV 中对应项相同值） |

三个服务的 token 值必须来自同一份部署配置（同一 CSV 的对应项）；值不一致
会在 token-service 验证 token 时以 401 暴露（不会静默降级）。

## 3. 启动顺序与依赖

```text
MySQL/PostgreSQL + Redis
        │
        ▼
token-service        ← 必须最先就绪：启动时 seed 身份 + 系统 Grant，
        │               校验 iam-service 主体存在，缺任何一项拒绝启动
        ▼
iam-service          ← 启动即拉 JWKS（30s 重试窗口）；迁移自动应用 V001~V012
        │
        ▼
（其他服务 / api-gateway）
        │
        ▼
用户访问 Bootstrap API（POST /v1/bootstrap，携带 X-LMMS-Init-Password）
```

- `docker compose` 中 gateway 已 `depends_on: token-service`；建议以
  `docker compose up -d token-service` 先行或整体 `up` 后对 iam 执行健康
  检查再发起 Bootstrap。
- token-service 未就绪时发起 Bootstrap：S05 失败（依赖不可用，HTTP 502），
  已完成步骤保留；**不会自动重试**——待 token-service 就绪后由用户再次
  访问同一 Bootstrap URL 显式恢复（设计 Q7/Q8）。

## 4. Bootstrap Step 与内部身份的关系（S05 职责）

- **S05 `internal_identities` 是 readiness / verify 步骤，不创建身份**。
  身份的实际创建发生在 token-service 启动 seed（第 1 节）；这是唯一责任
  方，避免 Bootstrap 与启动机制竞争创建同一 owner（重复身份/冲突凭据）。
- S05 检查项：`iam-service`、`notification-service` 两个 owner 在
  token-service 中存在且 `status = 'active'`（内部清单 = `SYSTEM_GRANTS`
  owner 并集，代码持有，不来自请求）。
- S06/S07 调用 `POST /internal/bootstrap/ensure-grants`（service-jwt
  `aud=token-service` 准入）：幂等补插缺失的系统 Grant；**已存在但被禁用
  的行不复活**，以 conflict 上报，Bootstrap 该步骤失败（fail-closed）。
- 全部 7 个 Step 完成后 Bootstrap 总状态 `completed`，接口永久 404。

## 5. 内部身份缺失 / 冲突的排查与恢复

| 症状 | 根因 | 处理 |
| --- | --- | --- |
| Bootstrap S05 失败，`last_error = "required service identity not ready"` | token-service 未启动、迁移未跑、或 `SERVICE_BOOTSTRAP_TOKENS/OWNERS` 漏配对应 owner | 1. 检查 token-service 健康与启动日志；2. 补齐 env CSV 后重启 token-service（per-owner 幂等补插，不影响已存在身份）；3. 用户再次访问 Bootstrap URL 恢复 |
| token-service 启动即退出，日志 `service-jwt grant principal validation failed` | CSV 中缺 `iam-service` 主体 | 补齐 CSV（iam-service 必须在列）后重启 |
| token-service 启动即退出，日志 `resolve seed pairs failed` | TOKENS 与 OWNERS 长度不等 | 对齐两个 CSV 后重启 |
| S06/S07 失败，`last_error = "step_conflict"` | 系统 Grant 已存在但被人工禁用 | 确认禁用是否人为意图：若误禁，人工将 `token_service_jwt_grants` 对应行恢复 `status='active'`（system_managed 行受管理面 API 保护，需 DBA 直接处置），再恢复 Bootstrap |
| Bootstrap 完成后需要轮换服务 token | 与 Bootstrap 无关 | 走 token-service 既有 rotate 管理接口（`/admin/bootstrap-tokens/{owner}/rotate`），Bootstrap 不参与轮换 |

## 6. 密钥轮换与冲突处理边界

- Bootstrap **不做**任何凭据生成、轮换或返回：S05 只读，S06/S07 只补插
  Grant 行（无凭据材料）。服务凭据轮换走既有管理面。
- `system_managed = TRUE` 的 Grant 行不受普通管理面 CRUD 影响
  （条件 UPDATE/DELETE 在 SQL 内原子保护）；人工禁用/恢复属于 DBA 直接
  数据库处置，处置期间 Bootstrap 对应步骤 fail-closed。
- 内部身份（bootstrap token）在两个服务侧值不一致时，唯一表现是
  service-jwt 签发 401；修正部署配置使两侧同源即可，无需 Bootstrap 参与。

## 7. 全新部署验证

集成测试 `services/token-service/tests/bootstrap_fresh_deploy_test.rs`
覆盖：全新空库 → 启动 seed（身份 + Grant + 主体校验）→ readiness 全绿 →
ensure-grants 幂等且无冲突 → 禁用行以 conflict 上报且不被覆盖 → 身份缺失
被 readiness 暴露 → iam 主体缺失时启动校验拒绝 → 二次启动 seed 幂等。

## 8. 更新记录

- 2026-11-28（WP-BOOTSTRAP-01-R1 / P2-01）：初版。明确 S05 readiness 职责
  与 token-service 启动 seed 的权威创建责任，补齐排查/恢复路径。
