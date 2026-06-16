# Tasks

- [x] Task 1: 网关路由确认与补全
  - [x] 1.1 检查 `docker/config/api-gateway/routes/` 是否已暴露 `/audit/*` 与 `/iam/*`，缺失则补 `audit.router.toml` / `iam.router.toml`
  - [x] 1.2 用 `cargo run -p api-gateway -- validate --routes ...` 校验

- [x] Task 2: 前端 API 模块
  - [x] 2.1 新增 `frontend/src/api/audit.ts`：operation/login/retention 接口与类型
  - [x] 2.2 新增 `frontend/src/api/iam.ts`：users/roles/permissions/menus/org-units 接口与类型

- [x] Task 3: 路由与菜单注册
  - [x] 3.1 在 `router/enums.ts` 增加 `monitor` rank 常量（项目原已有）
  - [x] 3.2 新建 `router/modules/monitor.ts`：operation-log / login-log / retention
  - [x] 3.3 修改 `router/modules/system.ts`：增加 permission 子路由
  - [x] 3.4 更新 `locales/zh-CN.yaml` 与 `en.yaml`：menus.monitor* / systemPermission

- [x] Task 4: 监控-操作日志页面
  - [x] 4.1 `views/monitor/operation-log/index.vue` 列表+筛选
  - [x] 4.2 行点击抽屉展示 before/after/ip/ua/request_id
  - [x] 4.3 `utils/hook.tsx` 数据装配

- [x] Task 5: 监控-登录日志页面
  - [x] 5.1 `views/monitor/login-log/index.vue` 列表+分页
  - [x] 5.2 success 标签、fail_reason 列
  - [x] 5.3 hook + columns

- [x] Task 6: 监控-数据保留页面
  - [x] 6.1 `views/monitor/retention/index.vue`：统计卡片 + 策略表单
  - [x] 6.2 保存策略 PUT
  - [x] 6.3 立即清理按钮 + 确认弹窗 + 结果提示

- [x] Task 7: 系统-权限管理页面
  - [x] 7.1 `views/system/permission/index.vue` 列表/筛选
  - [x] 7.2 `form.vue` + 新增/编辑弹窗
  - [x] 7.3 删除确认

- [x] Task 8: 系统-用户管理对接 iam
  - [x] 8.1 `views/system/user/utils/hook.tsx` 替换 mock 调用为 `/iam/users`
  - [x] 8.2 表单字段适配 iam 模型
  - [x] 8.3 角色绑定改用 iam 接口

- [x] Task 9: 系统-角色管理对接 iam
  - [x] 9.1 列表 CRUD 改用 `/iam/roles`
  - [x] 9.2 绑定权限改用 `PUT /iam/roles/:id/permissions`

- [x] Task 10: 系统-菜单管理对接 iam
  - [x] 10.1 列表/树/CRUD 切到 `/iam/menus`

- [x] Task 11: 系统-部门管理对接 iam org-units
  - [x] 11.1 列表/树/CRUD 切到 `/iam/org-units`

- [x] Task 12: 验证与构建
  - [x] 12.1 `pnpm -C frontend lint:eslint` 退出码 0
  - [x] 12.2 `pnpm -C frontend typecheck` 仅历史遗留错误，本次新增 / 改造文件无 TS 错误

# Task Dependencies
- Task 4/5/6 依赖 Task 2.1、Task 3
- Task 7/8/9/10/11 依赖 Task 2.2、Task 3
- Task 12 依赖前述全部完成
