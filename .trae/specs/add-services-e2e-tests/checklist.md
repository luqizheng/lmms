# Checklist

- [x] `tests/e2e/audit.e2e.test.ts` 存在并覆盖 operation/login/retention 全部场景
- [x] `tests/e2e/config.e2e.test.ts` 存在并覆盖 namespace+item CRUD 与 watch
- [x] `tests/e2e/file.e2e.test.ts` 存在并覆盖 upload/list/info/presign/delete
- [x] `tests/e2e/notification.e2e.test.ts` 存在并覆盖 announcement 全生命周期
- [x] `tests/e2e/token-admin.e2e.test.ts` 存在并覆盖 oauth-client CRUD + JWKS + user-jwt
- [x] `tests/e2e/iam-menus.e2e.test.ts` 与 `iam-org-units.e2e.test.ts` 存在并覆盖 CRUD
- [x] `tests/e2e/gateway.e2e.test.ts` 存在并覆盖健康路由与 401 拦截
- [x] `fixtures/payload.ts` 与 `helpers/ids.ts` 已扩展对应工厂与跟踪函数
- [x] `tests/e2e/package.json` 含全部新增分组脚本
- [x] `tests/e2e/README.md` 已同步文件结构与分组命令
- [x] 全部新增用例使用 `authedReq`/`baseURL`，未直接拼接绝对地址（除 ws）
- [x] 失败容忍：服务不可用时返回 502/404 等的用例使用 `expect([...]).toContain(res.status)` 而非硬断言
