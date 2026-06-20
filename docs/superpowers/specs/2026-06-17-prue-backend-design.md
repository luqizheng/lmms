# prue_backend 服务设计

## 目标

创建独立 Rust 服务 `prue_backend`。

服务用于适配 `frontend` 中的 pure-admin 后台。

核心能力是管理菜单配置和动态路由。

用户、角色只作为字符串字段保存。

服务不关联 `iam-service`。

前端用户、角色仍读取 iam 相关接口。

## 外部参考

- pure-admin 文档：https://pure-admin.cn/pages/introduction/
- pure-admin 后端参考：https://github.com/pure-admin/pure-admin-backend

## 服务边界

`prue_backend` 负责：

- 保存 pure-admin 菜单配置
- 管理菜单配置版本
- 发布一个 active 菜单版本
- 输出 `/get-async-routes`
- 输出 pure-admin 兼容响应
- 校验菜单 JSON 结构

`prue_backend` 不负责：

- 用户管理
- 角色管理
- 权限实体管理
- 调用 iam-service
- 同步 iam 数据

## 持久化方案

采用数据库 JSON 持久化。

菜单配置按版本保存。

每个版本保存完整菜单树快照。

只允许一个版本为 active。

推荐表：`menu_configs`。

字段：

- `id`: 字符串主键
- `version`: 版本号
- `name`: 配置名称
- `description`: 描述
- `routes_json`: 菜单路由树 JSON
- `status`: `draft` / `published` / `archived`
- `is_active`: 是否当前生效
- `created_by`: 创建人字符串
- `created_at`: 创建时间
- `updated_at`: 更新时间
- `published_at`: 发布时间

## JSON 合理性评估

JSON 持久化合理。

pure-admin 菜单本身就是路由树。

版本回滚只需切换 active 版本。

不需要多表递归查询。

不需要和 iam 菜单结构耦合。

适合配置型后台。

约束：

- 保存时必须校验结构
- 同一版本内 `path` 唯一
- 同一版本内 `name` 唯一
- `children` 必须是数组
- `meta.roles` 必须是字符串数组
- `meta.auths` 必须是字符串数组
- 发布时事务切换 active

## 路由 JSON 结构

`routes_json` 保存 pure-admin 动态路由格式。

示例：

```json
[
  {
    "path": "/system",
    "name": "System",
    "meta": {
      "title": "menus.pureSysManagement",
      "icon": "ri:settings-3-line",
      "rank": 1,
      "roles": ["admin"]
    },
    "children": [
      {
        "path": "/system/menu/index",
        "name": "SystemMenu",
        "component": "system/menu/index",
        "meta": {
          "title": "menus.pureSystemMenu",
          "icon": "ep:menu",
          "roles": ["admin"],
          "auths": ["menu:list", "menu:create"]
        }
      }
    ]
  }
]
```

## API 设计

统一响应：

```json
{
  "code": 0,
  "message": "ok",
  "data": null
}
```

### 动态路由

`GET /get-async-routes`

读取 active 版本。

返回：

```json
{
  "code": 0,
  "message": "ok",
  "data": []
}
```

### 菜单配置版本

`GET /pure/menu-configs`

查询配置版本列表。

`POST /pure/menu-configs`

新建草稿版本。

`GET /pure/menu-configs/{id}`

查看版本详情。

`PUT /pure/menu-configs/{id}`

更新草稿版本。

`POST /pure/menu-configs/{id}/publish`

发布为 active 版本。

`POST /pure/menu-configs/{id}/clone`

基于历史版本复制新草稿。

`DELETE /pure/menu-configs/{id}`

删除未发布版本。

### 系统端点

`GET /health`

健康检查。

`GET /metrics`

Prometheus 指标。

## 数据流

前端启动时调用 `/get-async-routes`。

api-gateway 转发到 `prue_backend`。

`prue_backend` 查询 active 菜单版本。

服务返回 pure-admin 路由树。

前端按现有逻辑注册动态路由。

## iam-service 移除范围

从 `iam-service` 移除菜单职责：

- `src/routes/menus.rs`
- `src/dto/menu.rs`
- `src/models/menu.rs`
- `src/service/menu_svc.rs`
- routes 模块中的 menus 注册
- dto 模块中的 menu 导出
- models 模块中的 menu 导出
- service 模块中的 menu_svc 导出
- openapi 中 menus paths 和 schemas
- README 菜单说明
- AGENTS 菜单端点说明
- iam 菜单测试
- api-gateway `/iam/menus/**` 路由

历史迁移文件不回改。

新部署不再依赖 iam 菜单接口。

## 新服务文件

新增目录：`services/prue_backend`。

需要包含：

- `Cargo.toml`
- `Dockerfile`
- `README.md`
- `AGENTS.md`
- `src/main.rs`
- `src/lib.rs`
- `src/config.rs`
- `src/routes/mod.rs`
- `src/routes/menu_configs.rs`
- `src/dto/menu_config.rs`
- `src/service/menu_config_svc.rs`
- `src/db.rs`
- `migrations/mysql/V001__init.sql`
- `migrations/sqlite/V001__init.sql`

`services/prue_backend/AGENTS.md` 必须写入 pure-admin 文档链接。

## 网关路由

新增 prue_backend 路由配置。

建议匹配：

- `/get-async-routes`
- `/pure/**`
- `/prue/health`

服务内部端口为 8080。

外部端口建议使用 6004。

## 前端改动

保留当前 pure-admin 动态路由逻辑。

`frontend/src/api/routes.ts` 继续调用 `/get-async-routes`。

若网关统一代理，无需改前端路径。

菜单配置管理页面后续再接入 `/pure/menu-configs`。

## 验证

实现后需要验证：

- `GET /get-async-routes` 返回 pure-admin 格式
- 无 active 版本时返回空数组
- 发布版本后只存在一个 active
- clone 生成草稿版本
- draft 可更新
- published 不允许直接更新
- path/name 重复会拒绝保存
- iam-service 不再暴露 `/iam/menus/*`
- api-gateway 不再转发 `/iam/menus/**`

## 风险

JSON 结构灵活，但数据库无法强约束字段。

需要在服务层做严格校验。

菜单查询简单，复杂筛选较弱。

当前需求以配置版本为主，可以接受。
