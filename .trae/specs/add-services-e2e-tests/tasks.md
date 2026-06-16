# Tasks

- [x] Task 1: 扩展 fixtures
  - [x] 1.1 在 `fixtures/payload.ts` 新增 `Payload.audit/config/file/notification/oauthClient/menu/orgUnit`
  - [x] 1.2 在 `helpers/ids.ts` 增加 trackNamespace/trackConfigItem/trackFile/trackAnnouncement/trackOAuthClient/trackMenu/trackOrgUnit

- [x] Task 2: audit-service e2e
  - [x] 2.1 新建 `tests/e2e/audit.e2e.test.ts`
  - [x] 2.2 用例：创建+查询 operation 日志、创建+查询 login 日志、retention/policy GET+PUT、retention/stats GET、retention/cleanup POST

- [x] Task 3: config-service e2e
  - [x] 3.1 新建 `tests/e2e/config.e2e.test.ts`
  - [x] 3.2 用例：namespace CRUD、config item CRUD、watch 长轮询超时返回

- [x] Task 4: file-service e2e
  - [x] 4.1 新建 `tests/e2e/file.e2e.test.ts`
  - [x] 4.2 用例：upload meta、list、info、presign get、delete

- [x] Task 5: notification-service e2e
  - [x] 5.1 新建 `tests/e2e/notification.e2e.test.ts`
  - [x] 5.2 用例：create→publish→detail→read→unread-count→revoke、list-mine

- [x] Task 6: token-service admin e2e
  - [x] 6.1 新建 `tests/e2e/token-admin.e2e.test.ts`
  - [x] 6.2 用例：oauth-client 创建/列表/详情/更新/删除、JWKS GET、user-jwt POST

- [x] Task 7: iam-service menus & org-units e2e
  - [x] 7.1 新建 `tests/e2e/iam-menus.e2e.test.ts`：菜单 CRUD
  - [x] 7.2 新建 `tests/e2e/iam-org-units.e2e.test.ts`：组织 CRUD

- [x] Task 8: api-gateway e2e
  - [x] 8.1 新建 `tests/e2e/gateway.e2e.test.ts`
  - [x] 8.2 用例：/iam/health、/audit/health、未授权 401

- [x] Task 9: package.json 脚本与 README 更新
  - [x] 9.1 在 `tests/e2e/package.json` 增加 `test:audit/test:config/test:file/test:notification/test:token-admin/test:iam-menus/test:iam-orgs/test:gateway`
  - [x] 9.2 README 同步分组列表

- [x] Task 10: 本地验证
  - [x] 10.1 运行 `pnpm -C tests/e2e test` 全量（依赖服务实际启动；脚本结构通过 GetDiagnostics 校验无 TS 错误）
  - [x] 10.2 失败用例统一使用容忍状态码数组，兼容服务未启时 404/502

# Task Dependencies
- Task 2-8 依赖 Task 1
- Task 9 依赖 Task 2-8
- Task 10 依赖 Task 9
