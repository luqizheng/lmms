# frontend i18n 硬编码清理 plan

## Status (2026-07-09)

### Key parity (本期 commit `xxx`)

en.yaml / zh-CN.yaml 现在完全对称，**各 288 keys，0 diff**。补的 16 key 全是 notification / announcement 子树：

```
notificationUnread / notificationRead / notificationDraft
notificationPublished / notificationRevoked
notificationMarkRead / notificationMarkAllRead / notificationRefresh
announcement / announcementTitle / announcementContent
announcementPublishTime / announcementExpireTime
announcementPinned / announcementPopover / announcementRecipients
```

### 硬编码债

非 pure-admin demo views 里直接写中文 template 字符串的 **577 行**（扫 `src/views/**` 排除了 `about/able/empty/error/guide/list/menuoverflow/nested/result/table/tabs` 这 11 个 pure-admin demo dir）。

主要集中：

| view | 行数（估算） |
|---|---:|
| notification/admin/index.vue | ~30 |
| notification/admin/form.vue | ~30 |
| notification/inbox/index.vue | ~40 |
| system/* | ~80 |
| monitor/* | ~50 |
| file/* / config/* / account-settings/* | ~80 |
| 其他 | ~250 |

## 为什么没在本期一次改完

- 577 行单 commit 太大，混 review 困难
- 部分 view 的中文还可能是后端返回字段（不需翻译）
- i18n 改造需要 view by view 验证渲染

## 分阶段推进（推荐顺序）

### Phase 1: 高频业务 view（约 200 行）

按「用户高频使用」优先：

1. `notification/admin/index.vue` + `form.vue` — 公告管理（已加 en keys，正好用上）
2. `notification/inbox/index.vue` — 收件箱
3. `system/user/index.vue` + 周边 — 用户管理
4. `account-settings/*` — 账号设置

每 view 一个 commit，配套 view 的 `.test.ts` 验证 `t()` 调用。

### Phase 2: 配置 / 文件 / 审计 view（约 200 行）

`config/*` / `file/*` / `monitor/*` — 同样 view by view。

### Phase 3: 写自动化守护

`scripts/check-i18n-hardcode.js`：

- 扫 `src/views/**/*.vue` template 块
- 检测直接中文（连续 ≥2 个 CJK）作为硬编码候选
- 跑在 `pnpm run check:frontend-i18n`（CI 失败 = fail）
- 豁免：注释、`<!-- -->`、已存在的 `$t()` 模板字符串内

只对**新增 view** 严格（避免历史债爆炸），老 view 视 Phase 1/2 进度逐步消化。

## 关键决策

**i18n key 命名**：保持当前 flat-with-prefix 风格（`notificationUnread` 而非 `notification.unread`），与既有 `pureDashboard` / `pureSystemMenu` 等一致。如未来要拆嵌套 namespace（`notification.status.unread`），开 v2 migration。

**翻译来源**：en 由开发者手写，zh-CN 沿用既有中文文案。新增 key 必须 en + zh-CN 同时写，否则 `check-frontend-i18n` fail。

**多语言**：当前只 en + zh-CN。新语言走 `@intlify/unplugin-vue-i18n` 已支持，复制 zh-CN 翻译即可。