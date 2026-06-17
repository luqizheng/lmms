# prue_backend 服务规范

## 外部文档

- pure-admin 文档：https://pure-admin.cn/pages/introduction/
- pure-admin 后端参考：https://github.com/pure-admin/pure-admin-backend

## 必需组件

- [x] Dockerfile
- [x] 健康检查端点（/health、/metrics）
- [x] 数据库迁移文件（migrations/）
- [x] README.md
- [x] tests/ 目录

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| DATABASE_URL | sqlite::memory: | 数据库连接 |
| SERVER_HOST | 0.0.0.0 | 服务监听主机 |
| SERVER_PORT | 8080 | 服务监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址 |
| RUST_LOG | info | 日志级别 |

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

## 数据库方言

支持 MySQL 和 SQLite。
