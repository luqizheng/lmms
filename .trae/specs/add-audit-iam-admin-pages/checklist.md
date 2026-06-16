# Checklist

- [x] `frontend/src/api/audit.ts` 暴露 operation/login/retention 全部接口与类型
- [x] `frontend/src/api/iam.ts` 暴露 users/roles/permissions/menus/org-units 全部接口与类型
- [x] `router/modules/monitor.ts` 注册 3 个监控子路由并在侧边菜单可见
- [x] `router/modules/system.ts` 注册 permission 子路由
- [x] `locales/zh-CN.yaml` 与 `en.yaml` 新增对应 menus.* 翻译
- [x] `/monitor/operation-log` 调用 `GET /audit/operation` 列表，支持筛选/分页/详情抽屉
- [x] `/monitor/login-log` 调用 `GET /audit/login` 列表并分页
- [x] `/monitor/retention` 展示 stats、可保存 policy、可手动 cleanup
- [x] `/system/permission` 完成 CRUD 联调
- [x] `/system/user` 列表/新增/编辑/删除调用 `/iam/users`
- [x] `/system/role` CRUD 与绑定权限调用 `/iam/roles[/:id]`
- [x] `/system/menu` 树/CRUD 调用 `/iam/menus[/:id]`
- [x] `/system/dept` 树/CRUD 调用 `/iam/org-units[/:id]`
- [x] api-gateway `audit.router.toml` 已新增并暴露 `/audit/*`
- [x] `pnpm -C frontend lint:eslint` 退出码 0
- [x] `pnpm -C frontend typecheck` 本次涉及文件无新增 TS 错误
