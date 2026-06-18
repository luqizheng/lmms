# TODO

## 菜单配置草稿并发控制

**What**：为 `prue_backend` 的菜单配置草稿保存增加冲突检测。

**Why**：两个管理员同时编辑同一个 draft 时，后保存者会覆盖前者的 `routes_json`。

**Pros**：避免多人协作时静默丢数据，提升后台配置安全性。

**Cons**：需要后端支持基于 `updated_at` 或版本号的条件更新，前端也要处理冲突提示与刷新。

**Context**：当前菜单编辑保存粒度是整体 PUT `routes_json`，不是节点级 patch。本轮只做现有 UI 到 `prue_backend` 的接口适配，先保证单人编辑闭环。

**Depends on / blocked by**：出现多人同时编辑需求，或后端增加条件更新能力。

## sanitize active routes 输出

**What**：在 `GET /get-async-routes` 输出前移除 `_menu` 等菜单管理兼容字段。

**Why**：菜单管理页需要 `_menu` 字段保留 `id`、`parentId`、`menuType`，但动态路由消费方不需要这些字段。

**Pros**：减少 active routes 响应污染，避免未知字段影响未来路由处理逻辑。

**Cons**：需要修改 `prue_backend` 输出逻辑，并补充后端测试。

**Context**：本轮为最小改动，允许 `_menu` 写入 `routes_json`，后端校验允许额外字段。当前假设前端动态路由忽略未知字段。

**Depends on / blocked by**：若发现动态路由消费方受 `_menu` 影响，再优先处理。
