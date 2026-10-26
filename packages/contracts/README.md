# @digua/contracts

后端契约的编译产物。

| 内容 | 真源 | 生成方式 |
|---|---|---|
| `ErrorCode` / `ErrorMessages` | `contracts/error-codes.yaml` | `contracts/codegen/gen-ts.js` 自动生成 |
| `ErrorMessagesZhCN` | 镜像 `libs/common-error/src/codes.rs` | 手写（便于产品文案评审） |
| `PermissionCode` / `PermissionDescriptions` | `contracts/permissions.yaml` | `contracts/codegen/gen-ts.js` 自动生成 |

**不要手改生成物**：改动 `contracts/*.yaml` 后运行 `pnpm run contracts:gen`，由
`pnpm run contracts:check` 校验漂移。

发布到私有 registry：`http://npm.kudi.repos/`。
