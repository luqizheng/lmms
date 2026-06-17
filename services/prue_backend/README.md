# prue_backend

pure-admin 后台适配服务。

## 功能

- 管理菜单配置版本
- JSON 持久化 pure-admin 路由树
- 发布 active 菜单版本
- 提供 `/get-async-routes`

## 边界

用户和角色只保存字符串数组。

服务不关联 iam-service。

## 运行

```powershell
$env:DATABASE_URL="sqlite::memory:"
$env:SERVER_ADDR="0.0.0.0:8080"
prue_backend
```
