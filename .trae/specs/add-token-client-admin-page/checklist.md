# Checklist

- [ ] `oauth_client_svc` 提供 list/update/delete/reset_secret 函数并通过 cargo build
- [ ] `routes/admin.rs` 暴露 5 个管理接口且响应不含 client_secret_hash
- [ ] 创建客户端接口仅在响应体中返回一次明文 client_secret
- [ ] api-gateway routes 校验通过（无 duplicate match）
- [ ] `frontend/src/api/token.ts` 新增 5 个管理 API 并带 TS 类型
- [ ] `views/system/token-client/index.vue` 展示列表、筛选、分页、操作列
- [ ] 创建成功弹窗显示 client_secret 且支持复制
- [ ] 系统菜单出现"令牌客户端"项，路由 `/system/token-client` 可访问
- [ ] zh-CN/en 文案均添加 `menus.systemTokenClient`
- [ ] 端到端：新增→编辑→禁用→重置 secret→删除 全流程成功
