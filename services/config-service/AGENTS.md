# config-service 服务规范

## 必需组件

- [x] Dockerfile
- [x] AGENTS.md
- [x] README.md
- [x] tests/ 目录
- [x] /health 端点
- [x] /metrics 端点
- [x] migrations/ 目录
- [x] Swagger 配置（SWAGGER_ENABLED 开关）

## 环境变量

配置走 `AppConfig::from_env()` 纯 env 模式（无 yaml 文件）。**`DATABASE_URL` 必填**（无 fallback，缺失或空白启动 fail-fast）。

| 变量 | 默认值 | 说明 |
|------|--------|------|
| DATABASE_URL | **必填** | 数据库连接串（`mysql://` 或 `postgres://`，按 URL 前缀判方言）；缺失或空白启动 fail-fast |
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8081 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8081 | 完整监听地址 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | false | 是否挂 SwaggerUi |

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /config/namespace | GET / POST | 命名空间 CRUD |
| /config/namespace/:id | GET / PUT / DELETE | 命名空间单操作 |
| /config/item | GET / POST | 配置项查询 / 创建（body 含 `is_secret` 字段） |
| /config/item/:id | GET / PUT / DELETE | 配置项单操作 |
| /config/export/:namespace_id | GET | 整 namespace 导出 JSON（含所有明文）|
| /config/import/:namespace_id | POST | 整 namespace 导入（强制 `is_secret=false`，调用方需自行设置）|
| /config/watch | GET (SSE) | 配置变更订阅 |
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true）|

## 敏感字段掩码（reveal policy，已落地）

`ConfigItem.is_secret=true` 的配置项对无 `CFG_CONFIG_REVEAL` 权限的调用方**强制掩码**：value 置为 `MASK = "********"`，`value_masked=true`。持有该权限的调用方返回明文。

- 权限码注册：`contracts/permissions.yaml:70`（`CFG_CONFIG_REVEAL`），Rust 常量 `libs/digua_authz/src/perms.rs:57`。
- 实现：`src/routes/config.rs` 内 `apply_reveal_policy(&mut item, &principal)` + 常量 `REVEAL_PERM`，在以下 5 个 handler 调用点应用：
  - `create_config_item`（POST /config/item 返回值）
  - `query_config_items`（GET /config/item 列表中每个 item）
  - `get_config_item`（GET /config/item/{id}）
  - `update_config_item`（PUT /config/item/{id} 返回更新后回读值）
  - `export_config`（GET /config/export/{namespace_id} 整 namespace 导出）
- 不应用点：`delete_config_item`（删除不返回 value）、`import_config`（强制 `is_secret=false`）。
- 契约测试：5 个 handler-context case 在 `src/routes/config.rs` `#[cfg(test)] mod tests`；4 个 baseline case（函数本身）在 `tests/reveal_policy_test.rs`。
- 默认绑定：仅 `platform_admin` / `tenant_admin` 角色自动拥有（`services/iam-service/migrations/mysql/V005__platform_admin_role.sql` + `V004__tenant_business_tables.sql` seed）。普通用户默认无此权限——**前端掩码不是唯一防线**，API 层会强制返回 MASK。


## 内部端点 /cfg/internal/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /cfg/internal/migrations/state | GET | schema 状态查询（Phase 10 G3）|

- 返回 2 个 V001-V002 migration 的 marker-table 探测结果
- 鉴权：service-jwt `aud=config-service`（Phase 10 G3 service-jwt 扩面）
- 详见 `docs-site/guides/migrations-state.md`

## 豁免

（无）

## 表命名约定

所有表必须以 `cfg_` 前缀（MySQL/PostgreSQL 共享 `superpowers` 数据库）。详见 `contracts/db-conventions.md`，CI 网关：`node scripts/check-db-naming.js`。

## AI 检查时机

修改 services/config-service/ 下文件时自动检查
