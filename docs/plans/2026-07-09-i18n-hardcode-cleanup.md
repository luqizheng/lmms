# frontend i18n 硬编码清理 plan

## Status (2026-07-10)

✅ **完成**。本 plan 三阶段全部落地。

- **Phase 1**（commit `b0ff011` ~ `8d1ff41`）：高频业务 view（notification/admin + form + inbox, system/user/dept/role/permission/token-client/tenant/session/menu, account-settings 4 文件）— 11 commits，~210 行
- **Phase 2**（commit `4fd8051` ~ `ed2faa8`）：config/file/monitor + system form 系列（config/namespace + item, file/index + presign + upload, monitor/retention + operation-log, system/dept/form + menu/form + user/form + token-client/api-keys）— 5 commits，~200 行
- **Phase 3**（commit `51e1004`）：`scripts/check-i18n-hardcode.js` 自动化守护 + 根 package.json wire
- **Phase 收尾**（commit `7fd12b4` + `e45ceb1`）：清理 12 view 48 处历史硬编码（`login`/`welcome`/`monitor/login-log`/`notification/detail`/`permission/button`/`permission/page` + 6 个 system/* 1-2 行的 openDialog 文案），加上 file/index.vue bulk delete 残留

### 最终 baseline

```
$ pnpm run check:frontend-i18n:report

i18n hardcode report (excluding pure-admin demo dirs)
  total views       : 23
  views w/ hardcode : 0
  total findings    : 0
```

`scripts/check-i18n-hardcode.js` 守好未来回归，新增 view 含硬编码中文即 CI fail。

### Key parity

i18n key 净增 299 对（en + zh-CN 各 +299），从 baseline 269 增至 712：
- Phase 1：+110
- Phase 2：+143
- Phase 收尾：+16
- Phase 3 自动化守护：+30（dashboard + notification 增强覆盖）

en + zh-CN 严格 1:1 同步（`pnpm run check:frontend-i18n` parity 验证）。

### 累计 commit（10 个 B 计划 commit）

```
b0ff011 feat(notification-admin): i18n-ize hardcoded strings + add view tests (B Phase 1.0)
8d1ff41 feat(notification-inbox): i18n-ize hardcoded strings + update test (B Phase 1.1)
fc0cab5 feat(system-user): i18n-ize hardcoded strings (B Phase 1.2)
2b83a9c feat(account-settings): i18n-ize index.vue tab labels + back button (B Phase 1.3)
0cb50ae feat(account-settings-components): i18n-ize Preferences + AccountManagement + SecurityLog (B Phase 1.4)
444fc0a feat(account-settings-profile): i18n-ize Profile.vue (B Phase 1.5)
700c9e8 feat(system-dept-role): i18n-ize dept + role index.vue (B Phase 1.6)
7cf978f feat(system-permission-token-client): i18n-ize 2 index.vue (B Phase 1.7)
27a2014 feat(system-tenant-session-menu): i18n-ize 3 index.vue + fix tests (B Phase 1.8)
3e846b3 feat(system-role-form): i18n-ize role/form.vue (B Phase 1 收尾)
4fd8051 feat(config): i18n-ize namespace + item views + fix tests (B Phase 2.1)
2d8188a feat(file): i18n-ize index + presign + fix tests (B Phase 2.2)
383d556 feat(monitor): i18n-ize retention + operation-log (B Phase 2.3)
7164ca3 feat(system-form): i18n-ize dept/form + menu/form (B Phase 2.4)
ed2faa8 feat(system-form-api-keys): i18n-ize user/form + token-client/api-keys (B Phase 2 收尾)
51e1004 feat(scripts): check-i18n-hardcode.js automation guard (B Phase 3)
7fd12b4 feat(i18n): clear historical hardcode debt (B 收尾) — 48 occurrences → 0
e45ceb1 fix(file): E Phase 2 收尾 — bulk delete 残留硬编码 i18n 化（债清零）
```

## 关键决策

**i18n key 命名**：保持当前 flat-with-prefix 风格（`notificationUnread` 而非 `notification.unread`），与既有 `pureDashboard` / `pureSystemMenu` 等一致。如未来要拆嵌套 namespace（`notification.status.unread`），开 v2 migration。

**翻译来源**：en 由开发者手写，zh-CN 沿用既有中文文案。新增 key 必须 en + zh-CN 同时写，否则 `check-frontend-i18n` fail。

**多语言**：当前只 en + zh-CN。新语言走 `@intlify/unplugin-vue-i18n` 已支持，复制 zh-CN 翻译即可。