# Frontend Trim Design

> 2026-06-10 · 把 `frontend/` 由 vue-pure-admin v7.0.0 完整版裁剪为"刚好够用"的极简骨架

## 1. 背景

`frontend/` 仓库是 https://github.com/pure-admin/vue-pure-admin 的 v7.0.0 完整克隆，包含 chatai、flow-chart、ganttastic、vueflow、ppt、mind、editor、markdown、formdesign、board、guide、menuoverflow、able、table、components、about、codemirror 等大批 demo 页，以及纯前端 mock 的 system/menu、system/dept、monitor 等页面。

后端实际能力（`docker/config/api-gateway/routes/*.router.toml` + 各服务 `routes.go`）只有 5 组：
- `auth-service`：`/api/auth/{register,login,logout}`
- `user-service`：`/api/users`（me/list/get/patchExtends）、`/api/users/:id/roles`、`/api/roles`（CRUD + assignPermissions）、`/api/permissions`（list）
- `token-server`：`/api/tokens/{access,refresh,revoke,resource,api_key}`
- 内部 `/internal/users/:id/claims` 仅网关使用，前端无需求

需要把前端裁剪成"登录 / 工作台 / 个人中心 / 用户 / 角色 / 权限 + admin 布局壳"的极简骨架。

## 2. 目标与非目标

### 2.1 目标
- 5 个一级菜单 + 1 个个人中心 + 3 个错误页
- 所有业务页接真后端；mock/ 目录与 vite-plugin-fake-server 全删
- 保留 i18n（zh-CN + en 双语）
- 保留布局壳（侧栏 / 顶栏 / 多页签 / 主题 / 全屏）
- 保留 `v-perms` / `v-auth` 指令，绑定后端返回的 roles → 权限码

### 2.2 非目标
- 不接 system/menu、system/dept（后端无菜单/部门 API）
- 不引入微前端 / 组件库抽离
- 不重构 vue-pure-admin 自身的 layout/store 框架

## 3. 路由结构

```text
/                                    布局壳
├── /welcome                          工作台（去除 mock 数据）
├── /system                           系统管理
│   ├── /system/user                  用户管理
│   ├── /system/role                  角色管理
│   └── /system/permission            权限列表
├── /account-settings                 个人中心
│   ├── /account-settings/profile     个人资料
│   └── /account-settings/security    安全日志（token 撤销记录）
├── /login                            登录页（公开）
└── /error/{403,404,500}              异常页
```

## 4. 目录处理清单

### 4.1 整目录 / 整文件删除

```text
mock/
public/audio/
public/wasm/
public/html/button.html
src/router/modules/able.ts
src/router/modules/about.ts
src/router/modules/board.ts
src/router/modules/chatai.ts
src/router/modules/codemirror.ts
src/router/modules/components.ts
src/router/modules/editor.ts
src/router/modules/flowchart.ts
src/router/modules/form.ts
src/router/modules/formdesign.ts
src/router/modules/ganttastic.ts
src/router/modules/guide.ts
src/router/modules/list.ts
src/router/modules/markdown.ts
src/router/modules/menuoverflow.ts
src/router/modules/mind.ts
src/router/modules/nested.ts
src/router/modules/ppt.ts
src/router/modules/result.ts
src/router/modules/table.ts
src/router/modules/vueflow.ts
src/views/able/
src/views/about/
src/views/chatai/
src/views/codemirror/
src/views/components/
src/views/editor/
src/views/flow-chart/
src/views/ganttastic/
src/views/guide/
src/views/list/
src/views/markdown/
src/views/menuoverflow/
src/views/mind/
src/views/monitor/
src/views/nested/
src/views/permission/
src/views/ppt/
src/views/result/
src/views/schema-form/
src/views/table/
src/views/tabs/
src/views/vue-flow/
src/views/empty/
src/views/login/components/LoginPhone.vue
src/views/login/components/LoginQrCode.vue
src/views/login/components/LoginRegist.vue
src/views/login/components/LoginUpdate.vue
src/plugins/vxeTable.ts
```

Re* 通用组件：除 `RePureTableBar`、`ReDialog`、`ReDrawer`、`RePerms`、`ReIcon`、`ReText` 外整目录删除。

### 4.2 改写

- `src/router/index.ts`：清理 `import.meta.glob` 与排序；只引入 `home/system/error` 三个模块；`account-settings` 走 children 形式挂在 `/` 下。
- `src/router/enums.ts`：只保留 `home / system / account / error` 四个 rank。
- `src/router/modules/{home,system,error}.ts` + 新增 `src/router/modules/account.ts`：按第 3 节定义。
- `src/views/welcome/index.vue`：去除 mock 数据源，保留 KPI 卡片骨架，初始状态从 `/api/users?page=1&size=5` 拉取最近注册用户作为动态数据。
- `src/views/system/user/**`：列表接 `GET /api/users`；新增 / 编辑 / 删除 / 重置密码 / 分配角色接对应端点；保留搜索 + 分页。
- `src/views/system/role/**`：列表接 `GET /api/roles`；CRUD 接对应端点；分配权限接 `POST /api/roles/:id/permissions`。
- `src/views/system/permission/**`：只读列表，接 `GET /api/permissions`。
- `src/views/system/{dept,menu}/**`：删除（后端无 API）。
- `src/views/login/index.vue`：账号密码登录接 `POST /api/auth/login`；去除手机 / 二维码 / 注册 / 找回密码 UI。
- `src/views/account-settings/components/Profile.vue`：接 `GET / PUT /api/users/me`。
- `src/views/account-settings/components/SecurityLog.vue`：改用 `/api/tokens/revoke` 记录 + 当前会话列表（前端从 localforage 读取 token 列表）。

### 4.3 保留

- `src/layout/**`：admin 布局壳（侧栏、顶栏、页签、全屏、设置面板）
- `src/store/modules/{app,epTheme,multiTags,permission,settings,user}.ts`
- `src/utils/{http,auth,mitt,message,propTypes}.ts`
- `src/plugins/{i18n,elementPlus,echarts}.ts`
- `src/directives/{auth,perms,copy}.ts`
- `src/components/Re{PureTableBar,Dialog,Drawer,Perms,Icon,Text}/`

## 5. 依赖裁剪

### 5.1 删除（dependencies）

```
@amap/amap-jsapi-loader
@howdyjs/mouse-menu
@infectoone/vue-ganttastic
@logicflow/core
@logicflow/extension
@pureadmin/descriptions
@pureadmin/table
@vue-flow/background
@vue-flow/core
@wangeditor/editor
@wangeditor/editor-for-vue
@zxcvbn-ts/core
animate.css
china-area-data
codemirror
codemirror-editor-vue3
cropperjs
deep-chat
el-table-infinite-scroll
highlight.js
intro.js
jsbarcode
mint-filter
mqtt
path-browserify
pinyin-pro
plus-pro-components
qrcode
sortablejs
swiper
typeit
v-contextmenu
vditor
version-rocket
vue-json-pretty
vue-pdf-embed
vue-tippy
vue-types
vue-virtual-scroller
vue-waterfall-plugin-next
vue3-danmaku
vue3-puzzle-vcode
vuedraggable
vxe-table
wavesurfer.js
xgplayer
xlsx
```

### 5.2 删除（devDependencies）

```
@types/codemirror
@types/dagre
@types/intro.js
@types/path-browserify
@types/qrcode
@types/sortablejs
@faker-js/faker
@types/dagre
dagre
vite-plugin-fake-server
```

### 5.3 保留

dependencies：`vue / vue-router / pinia / vue-i18n / element-plus / @vueuse/core / axios / js-cookie / mitt / nprogress / qs / responsive-storage / localforage / @iconify/vue / @pureadmin/utils / dayjs / echarts / @vueuse/motion`

devDependencies：`vite / @vitejs/plugin-vue / @vitejs/plugin-vue-jsx / typescript / vue-tsc / tailwindcss / @tailwindcss/vite / sass / unplugin-icons / @iconify/json / vite-plugin-cdn-import / vite-plugin-compression / vite-plugin-remove-console / vite-plugin-router-warn / vite-svg-loader / eslint* / prettier* / stylelint* / husky / lint-staged / code-inspector-plugin / rollup-plugin-visualizer / @types/{node,nprogress,js-cookie,qs}`

## 6. 后端 API 映射

| 后端 | 前端位置 |
|---|---|
| `POST /api/auth/login` | `src/api/auth.ts` Login() |
| `POST /api/auth/logout` | `src/api/auth.ts` Logout() |
| `POST /api/auth/register` | `src/api/auth.ts` Register()（暂不挂 UI，导出备用） |
| `GET /api/users/me` | `src/api/user.ts` getCurrentUser() |
| `PUT /api/users/me` | `src/api/user.ts` updateCurrentUser() |
| `GET /api/users` | `src/api/user.ts` listUsers() |
| `GET /api/users/:id` | `src/api/user.ts` getUser() |
| `PATCH /api/users/:id/extends` | `src/api/user.ts` patchUserExtends() |
| `POST /api/users/:id/roles` | `src/api/user.ts` assignUserRoles() |
| `DELETE /api/users/:id/roles` | `src/api/user.ts` removeUserRoles() |
| `GET /api/roles` | `src/api/role.ts` listRoles() |
| `POST /api/roles` | `src/api/role.ts` createRole() |
| `GET /api/roles/:id` | `src/api/role.ts` getRole() |
| `PUT /api/roles/:id` | `src/api/role.ts` updateRole() |
| `DELETE /api/roles/:id` | `src/api/role.ts` deleteRole() |
| `POST /api/roles/:id/permissions` | `src/api/role.ts` assignRolePermissions() |
| `GET /api/permissions` | `src/api/permission.ts` listPermissions() |
| `POST /api/tokens/access` | `src/api/token.ts` createAccessToken() |
| `POST /api/tokens/refresh` | `src/api/token.ts` refreshToken() |
| `POST /api/tokens/revoke` | `src/api/token.ts` revokeToken() |
| `POST /api/tokens/api_key` | `src/api/token.ts` createApiKey() |
| `GET /api/tokens/resource` | `src/api/token.ts` listResources() |

## 7. 鉴权适配

- 后端 JWT `Principal.Roles` 注入 `pinia userStore.roles`；
- `v-perms` 指令从 `userStore.roles → permissionCodes` 匹配；
- `v-auth` 保持原语义（角色码比较）；
- token 存 `localforage` 加密 key 下，刷新机制沿用现有 `utils/auth.ts`。

## 8. .env 与代理

```ini
# .env.production / .env.development
VITE_BASE_URL=/
VITE_PROXY_URL=
```

Nginx 反代规则（`docker/config/nginx/nginx.conf`）：

```nginx
location /auth/         { proxy_pass http://api-gateway:3000; }
location /users/        { proxy_pass http://api-gateway:3000; }
location /roles/        { proxy_pass http://api-gateway:3000; }
location /permissions/  { proxy_pass http://api-gateway:3000; }
location /tokens/       { proxy_pass http://api-gateway:3000; }
```

## 9. 验收

- `pnpm lint` 通过
- `pnpm typecheck` 通过
- `pnpm build` 产物 < 1.5 MB（gzip）
- 启动后无 mock 引用、无 404 资源引用
- 登录 → 工作台 → 用户管理 → 角色管理 → 权限列表 全链路通；权限按钮按角色显隐

## 10. 不做的事

- 不改 `services/` 任何后端代码
- 不动 `libs/` `sdk/` `docker/`
- 不动 `vite.config.ts` 中除 `vite-plugin-fake-server` 之外的插件
- 不删 `locales/zh-CN.yaml` 与 `locales/en.yaml`（双语保留）
- 不做单元测试
