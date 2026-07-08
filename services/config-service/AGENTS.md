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

| 变量 | 默认值 | 说明 |
|------|--------|------|
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8080 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址 |
| DATABASE_URL | 服务自定义 | 数据库连接 |
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

## 已知缺口 — 敏感字段掩码

`ConfigItem.is_secret` 字段已存在（`models/config_item.rs:13`），但当前 `get_config_item` / `query_config_items` / `export_config` 全部返回明文，未对 `CFG_CONFIG_REVEAL` 权限做强校验。**前端掩码仅为 UX**，调用方仍有 API 路径可读明文。后端需补：

- `get_config_item`：`is_secret=true` 时若 caller 无 `CFG_CONFIG_REVEAL`，返回 `value=""` + `value_masked=true` 字段；
- `query_config_items`：同上策略，列表场景；
- `export_config`：默认导出调用方无 reveal 时全部 mask，否则全明文。


## 内部端点 /cfg/internal/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /cfg/internal/migrations/state | GET | schema 状态查询（Phase 10 G3）|

- 返回 2 个 V001-V002 migration 的 marker-table 探测结果
- 鉴权：service-jwt `aud=config-service`（Phase 10 G3 service-jwt 扩面）
- 详见 `docs/guides/migrations-state.md`

## 豁免

（无）

## 表命名约定

所有表必须以 `cfg_` 前缀（共享 `superpowers` MySQL 数据库）。详见 `contracts/db-conventions.md`，CI 网关：`node scripts/check-db-naming.js`。

## AI 检查时机

修改 services/config-service/ 下文件时自动检查
