/**
 * @digua/contracts —— 后端契约的编译产物。
 *
 * error-codes.ts / permissions.ts 由 `contracts/codegen/gen-ts.js` 从
 * `contracts/*.yaml` 生成；error-codes.zh-CN.ts 是手写的中文文案镜像
 * （便于产品文案评审）。**均不得在业务代码里手改。**
 */
export * from "./error-codes.js";
export * from "./permissions.js";
export { ErrorMessagesZhCN } from "./error-codes.zh-CN.js";
