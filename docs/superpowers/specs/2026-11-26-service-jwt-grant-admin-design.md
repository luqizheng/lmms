# Service JWT Audience Grant 管理控制面设计（WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01）

- 日期：2026-11-26
- 基线：main @ 55e379ff（开工）→ 合并 PR #17 后的 main @ c152f33e（收尾）
- 任务书：WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01
- 状态：已按合并后的上游（WP-02）适配

## 0. 与任务书前提的关系（两次调查）

**开工调查（基线 55e379ff）**：任务书假设的"SERVICE-JWT-AUDIENCE 已有运行时
授权"在当时的 main 上不存在（无模型/表/检查/管理 API），本任务从零建立。

**开发中途上游变化**：PR #17（WP-LMMS-SERVICE-JWT-AUDIENCE-02，运行时实现）
于 2026-11-26 10:06 合入 main，与本分支全面撞车（同名 V004 迁移、同名模型/
测试文件）。**按任务书"以当前代码为准"原则适配**：merge 新 main，冲突全部
以上游为准；本分支收敛为上游缺失的增量——**授权管理控制面**（上游 WP-02
明确"本阶段不提供 grant CRUD"）。

## 1. 上游已存在（本任务直接复用，不再重复实现）

- `token_service_jwt_grants` 表（V004 + V005 状态感知 PK 迁移，rust_step）；
- 运行时检查：`is_allowed`（403 默认拒绝 / DB 错误 500 fail-closed）、
  `canonical_owner`（`#old` 宽限期归一）、`validate_service_identity`；
- code-owned 系统 seed：`SYSTEM_GRANTS`（iam→token P0、ntf→iam P1）+
  `ensure_system_grants` 幂等补插 + `validate_required_principals` 启动校验。

## 2. 本任务增量

### 2.1 管理面 CRUD（模型层追加，双入口共用）

`list_filtered`（分页 + owner/audience/status 过滤 + 稳定排序）、
`create_grant`（先查后插 + PK 冲突 → `Duplicate` 映射 409）、
`set_grant_status`（条件 UPDATE `AND status <> ?` 防 TOCTOU）、
`delete_grant`。安全语义：请求体不接受 system_managed（恒 false）；
`system_managed` 行禁改删（409）；page≥1、size 1..=100（越界 400）；
错误响应静态文案不泄露内部信息。

### 2.2 双入口 Controller

| 入口 | 路径 | 鉴权 |
|---|---|---|
| service | `/admin/service-jwt-grants*` | `verify_aud_principal_middleware("token-service")` |
| web | `/v1/admin/service-jwt-grants*` | user-jwt `Principal` + 权限码（运行时强制） |

权限码（contracts/permissions.yaml 契约，contracts-gen 生成 perms.rs /
permissions.ts）：`TOKEN_SERVICE_JWT_GRANT_READ/CREATE/UPDATE/DELETE`。

### 2.3 审计（V006 迁移，双方言）

append-only 表 `token_service_jwt_grant_audit`：actor_id/type、owner、
audience、action（create/update/enable/disable/delete）、before/after、
request_id（`X-Request-Id` 或 uuid）、result（success/denied）、occurred_at。
成功变更 + 系统保护拒绝 + web 权限拒绝均落行；永不携带凭据。
缺口（报告中说明）：token-service 无 MQ/outbox，审计为本库本地表；
变更与审计非同事务（`db_*!` 宏不支持事务 executor），审计写失败 → 500
（变更已生效，重试幂等）。

### 2.4 OpenAPI / 网关 / SDK / 测试

- 10 个 handler（5×2）注册 openapi.rs，重生成 openapi.yaml；
- 两份 `token.router.toml` 增加 `/v1/admin/service-jwt-grants*` 转发；
- SDK（未发版）：Rust `digua_token_sdk`（+transport put_json/delete，trait
  默认实现保持下游 mock 兼容；404/409 → Validation）、Java
  `digua-token-client-sdk`（POJO + 同步/异步）、npm `sdk/iam-services`
  （`TokensApi.serviceJwtGrants`）；
- 测试：`service_jwt_grant_admin_test.rs`（管理面 15 用例：双入口鉴权、
  CRUD 语义、system_managed 保护、并发创建恰一次、分页边界、两入口一致、
  审计落行、签发联动、DB 死亡 fail-closed）；`pg_roundtrip_test.rs` 扩展
  grants 生命周期（含上游 seed 幂等与保护行断言）。

## 3. 语义变化（相对本分支初版，随上游适配）

- **seed 语义改为上游**：不再有 7×7 dev 全矩阵 / `SERVICE_JWT_GRANT_SEED`
  env；初始授权 = 上游 2 条 code-owned 系统授权，其余通过管理 API 建立。
  交付说明：升级后未建 grant 的 owner→audience 将被拒签（默认拒绝）。
- 本分支不携带 V004（上游已发布）；审计表为 **V006**（forward-only 追加）。
- 运行时 403 文案沿用上游 "audience not granted"；签出 sub 用 canonical
  owner（上游）。

## 4. 不做

- 不改上游 V004/V005 迁移与 rust_step；不改 `SYSTEM_GRANTS`；
- 不执行 Registry 发布 / 生产迁移 / PR 合并 / force-push / amend。
