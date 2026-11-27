# WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-02 设计 — grant 管理面 P1 安全修复

日期：2026-11-27
基线：PR #19 @ `7caefd3e`（分支 `feat/wp-lmms-service-jwt-audience-admin-02`）
任务：修复 PR #19 安全审查提出的两个 P1 问题。

## P1-01 Service JWT 管理入口权限边界

### 风险

`/admin/service-jwt-grants*` 仅靠 `verify_aud_principal_middleware("token-service")`（验签 + aud）。
任何持有 `aud=token-service` service-jwt 的普通业务服务都能 CRUD 任意 owner 的 grant →
等价于拿到全局服务间授权管理能力。

### 修复模型

- **管理权限来源**：部署级管理员清单（与 `SERVICE_BOOTSTRAP_TOKENS` 同级信任）：
  - yaml `service_jwt_grant_admins` / env `TOKEN_SERVICE_JWT_GRANT_ADMINS`（逗号分隔
    canonical owner；空 = fail-closed，无人可管理）。
  - 存入 `Config::service_jwt_grant_admins` → `TokenServerState.service_jwt_grant_admins`。
- **调用者身份**：service-jwt `sub`（token-service 签发时从 DB bootstrap owner 写入，
  RS256 验签 + aud 强制保护）→ `Principal::user_id`。清单比对前做 `canonical_owner`
  归一（strip `#old`）。请求体 / 路径参数（owner、audience）只作为操作目标，
  **不参与**权限判定。
- **检查位置**：5 个 handler（list / get / create / set_status / delete）第一步，
  在任何业务读写与审计动作之前；拒绝 → 403，变更类 best-effort 记 denied 审计
  （与 Web 入口惯例一致），查询类只记 log。拒绝不产生任何业务数据变更。
- **Web UI 入口（`/v1/admin/*`，user-jwt + 权限码）不动**。
- 不新增权限码（service 入口不携带 permissions claim，且改签发协议被任务禁止）。

## P1-02 业务变更与审计写入原子性

### 风险

ops 层先执行 grant INSERT/UPDATE/DELETE，再独立写审计；审计失败 → 500 但业务已提交
（非原子）。模块文档自己也承认（`service_jwt_grant_admin.rs` 头注释）。

### 修复：真事务

- `libs/common-db`：激活既有死代码 `Transaction`（原为不可构造的 Option-struct，
  无任何使用点 → 重构为 enum 变体安全），补 `DbPool::begin()` / `commit()` /
  `rollback()`，并新增 `db_tx_execute!` / `db_tx_fetch_optional!` / `db_tx_fetch_scalar!` /
  `db_tx_fetch_all!` 宏族（与池级宏同款 render+bind 校验，executor 为 `&mut **t`）。
- token-service `models::service_jwt_grant`：新增 `*_tx` 变体（事务内执行），
  读加 `FOR UPDATE` 行锁消除先查后改 TOCTOU。
- ops 层成功路径全部改为单事务：
  - create：dup 检查 → INSERT grant → INSERT 审计(success) → commit；
  - set_status：SELECT FOR UPDATE → 条件 UPDATE → INSERT 审计(success) → commit；
  - delete：SELECT FOR UPDATE → DELETE → INSERT 审计(success) → commit。
  - 任一步失败 → 整体回滚（sqlx 事务 drop 自动回滚 + 显式 rollback）→ 500，
    grant 状态不变，无 success 审计行。
  - denied/404/权限拒绝路径：事务回滚后用池级 best-effort 审计（行为与现状一致，
    且事务边界内只含"业务变更 + 成功审计"）。
- system_managed 保护不变（条件 UPDATE/DELETE SQL 内原子 + FOR UPDATE 锁下判定）。
- MySQL / PostgreSQL 走同一份宏渲染 SQL，语义一致。

## 测试

- P1-01：非 admin service-jwt 对 5 个管理路由全 403 且无业务变更；admin 放行；
  wrong-aud / 未认证 401；改 owner/audience 参数不越权；直连内部路由同样被拒；
  Web UI 权限码回归（既有测试保持绿）；config 解析单元测试。
- P1-02（故障注入，MySQL + PG 双方言真实 DB）：
  - 注入方式：RENAME 审计表 → 审计 INSERT 必失败 → 断言 grant 未变 + 500 + 审计无
    success 行 → RENAME 还原（每测试独立数据库，互不影响）。
  - CREATE：grant INSERT 成功、审计失败 → grant 不存在。
  - UPDATE：状态保持原值。DELETE：grant 仍存在。
  - 正常路径：审计 before/after/actor/action/request_id 逐字段断言。
  - 并发：并发启停不绕过 system_managed；并发 create 恰一次成功。

## 文档 / 配置同步

- `services/token-service/AGENTS.md`：env 表 + 端点域说明。
- `services/token-service/README.md`：管理面鉴权说明。
- OpenAPI：handler 注解加 403 → `node scripts/dump-openapi-specs.js` 重生成。
- 无新增迁移（schema 不变）；无新增权限码（`contracts/permissions.yaml` 不动）。
