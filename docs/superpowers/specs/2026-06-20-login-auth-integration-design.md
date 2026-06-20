# Spec: 登录认证前后端打通（iam 自签）

## Objective

打通前端登录页到后端 iam-service 的认证链路，完成 **登录 / 刷新 / 登出** 闭环。
JWT 由 iam-service 内部自签，token-service 不参与登录流程。
附带：登录验证码可通过环境变量开关，便于 UI 自动化测试。

成功标志：
- 前端登录页输入用户名密码即可登录并跳转首页。
- 登出调用后端接口使 refresh token 失效。
- 三层路由（前端 axios / api-gateway / iam-service）路径对齐 `/api/v1/iam/auth/*`。
- `pnpm typecheck` 退出码 0。

## Tech Stack

- 前端：Vue3 + Vite + Pinia + Element Plus + axios（vue-pure-admin 模板）
- 后端：Rust + axum，iam-service（端口 6002，内部 8080）
- 网关：api-gateway（端口 3000）
- E2E：vitest + supertest（API 直连）

## Commands

- 类型检查：`pnpm -C frontend typecheck`
- 前端开发：`pnpm -C frontend dev`
- 网关路由校验：`cargo run -p api-gateway -- validate --routes infrastructure\config\api-gateway\routes`
- 起后端：`cd infrastructure; docker compose up -d`
- E2E（auth）：`node scripts/run-e2e-local.js auth`

## Project Structure

```
frontend/src/api/auth.ts            → 登录/刷新/登出/注册/setup-status 接口
frontend/src/api/role.ts            → 角色接口（PageResult 本地定义）
frontend/src/store/modules/user.ts  → 用户状态，loginByUsername/logOut/handRefreshToken
frontend/src/views/login/index.vue  → 登录页
frontend/src/views/login/utils/rule.ts → 登录表单校验规则、captchaDisabled 开关
frontend/src/views/login/register.vue  → 注册管理员页
frontend/src/router/modules/account.ts → 账号设置路由
frontend/vite.config.ts             → dev 代理 rewrite
frontend/.env.development           → VITE_DISABLE_CAPTCHA 开关
docker/config/api-gateway/routes/iam.router.toml        → 网关 iam 路由（dev）
infrastructure/config/api-gateway/routes/iam.router.toml → 网关 iam 路由（E2E）
services/iam-service/src/routes/auth.rs   → 后端 auth 路由
services/iam-service/src/service/auth_svc.rs → 后端登录逻辑
tests/e2e/auth.e2e.test.ts          → auth E2E 用例
```

## Code Style

axios 路径严格遵循 routing-rules：`/api/v1/<service>/<resource>`。

```ts
export function login(data: LoginRequest) {
  return http.request<LoginResult>("post", "/api/v1/iam/auth/login", { data });
}
```

环境变量开关：

```ts
export const captchaDisabled = import.meta.env.VITE_DISABLE_CAPTCHA === "true";
```

## Testing Strategy

- 前端：`pnpm typecheck` 必须 0 错误。
- 后端：不改，依赖既有逻辑。
- E2E：`tests/e2e/auth.e2e.test.ts` API 直连验证 login/refresh/logout/setup-status，不覆盖前端 UI。
- UI：手动或自动化在 dev 服务器登录页验证闭环（验证码开关 true 时免校验）。

## Boundaries

- Always：前端路径补 `iam` 段；改路由后 validate；保留验证码生产默认开启。
- Ask first：改后端业务源码；改 token-service 集成方式。
- Never：删除验证码功能；提交 secrets；改 services/libs/sdk 业务源码（本任务前端为主）。

## Success Criteria

1. `frontend/src/api/auth.ts` 五处路径均为 `/api/v1/iam/auth/*`。
2. `vite.config.ts` 代理含 `rewrite` 剥除 `/api`。
3. `store/modules/user.ts` `logOut` 调用 `logoutApi` 后清本地。
4. 验证码开关 `VITE_DISABLE_CAPTCHA` 生效（true 时登录页无验证码框、免校验）。
5. `pnpm typecheck` 退出码 0。
6. 网关路由 validate 通过，infrastructure 与 docker 路由含 `/v1/iam/auth/*`。
7. E2E auth 用例可运行（需 Docker 在线）。

## Open Questions

无。用户已确认：iam 自签、保留验证码（开关方式）、全自动执行。
