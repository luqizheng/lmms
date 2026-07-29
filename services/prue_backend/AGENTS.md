# prue_backend 服务规范

## 外部文档

- pure-admin 文档：https://pure-admin.cn/pages/introduction/
- pure-admin 后端参考：https://github.com/pure-admin/pure-admin-backend

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

配置走 `AppConfig::from_env()` 纯 env 模式（无 yaml 文件）。**`DATABASE_URL` 必填**（无 fallback，缺失或空白启动 fail-fast）。本服务不再支持默认 `sqlite::memory:`，测试或本地开发需显式设 `DATABASE_URL`。

| 变量 | 默认值 | 说明 |
|------|--------|------|
| DATABASE_URL | **必填** | MySQL 或 SQLite 连接串；缺失或空白启动 fail-fast |
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8080 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | false | 是否挂 SwaggerUi |

## 服务边界

- 只管理 pure-admin 菜单配置和动态路由。
- 菜单配置使用版本化 JSON 持久化。
- 用户和角色仅保存为字符串数组。
- 不关联 iam-service。
- 不管理用户、角色、权限实体。

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /get-async-routes | GET | 获取 active 菜单版本的 pure-admin 动态路由 |
| /pure/menu-configs | GET | 查询菜单配置版本列表 |
| /pure/menu-configs | POST | 新建草稿菜单配置 |
| /pure/menu-configs/{id} | GET | 查看菜单配置版本详情 |
| /pure/menu-configs/{id} | PUT | 更新草稿菜单配置 |
| /pure/menu-configs/{id}/publish | POST | 发布菜单配置为 active |
| /pure/menu-configs/{id}/clone | POST | 克隆历史版本为草稿 |
| /pure/menu-configs/{id} | DELETE | 删除未发布菜单配置 |
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true）|

## 内部端点 /prue/internal/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /prue/internal/migrations/state | GET | schema 状态查询（Phase 10 G3，双 dialect）|

- 返回 1 个 V001 migration × 2 dialects（mysql + sqlite）的 marker-table 探测结果
- `dialect` 字段返回 `"mysql+sqlite"`
- 鉴权：service-jwt `aud=prue_backend`（Phase 10 G3 service-jwt 扩面）
- 详见 `docs-site/guides/migrations-state.md`

## 豁免

（无）

## 数据库方言

支持 MySQL 和 SQLite。

## 表命名约定

所有表必须以 `prue_` 前缀（共享 `superpowers` MySQL 数据库）。详见 `contracts/db-conventions.md`，CI 网关：`node scripts/check-db-naming.js`。

## AI 检查时机

修改 services/prue_backend/ 下文件时自动检查
