// iam-services：lmms IAM SDK（登录 / 用户管理 / token 管理）。
// 零运行时依赖，原生 fetch，浏览器 / Node ≥18 通用；面向 api-gateway 的 /v1 公开路径。

import { AuthApi } from "./auth.js";
import { HttpCore, type IamServicesOptions } from "./http.js";
import { OrgUnitsApi } from "./org-units.js";
import { PermissionsApi } from "./permissions.js";
import { RolesApi } from "./roles.js";
import { TokensApi } from "./tokens.js";
import { UsersApi } from "./users.js";

export * from "./types.js";
export * from "./errors.js";
export { DEFAULT_BASE_URL, HttpCore, type IamServicesOptions, type RequestOptions } from "./http.js";
export { AuthApi } from "./auth.js";
export { UsersApi } from "./users.js";
export { RolesApi } from "./roles.js";
export { PermissionsApi } from "./permissions.js";
export { OrgUnitsApi } from "./org-units.js";
export { TokensApi, ApiKeysApi } from "./tokens.js";

export interface IamServices {
  /** 登录 / 刷新 / 登出 / 改密 / 会话管理（iam-service）。 */
  readonly auth: AuthApi;
  /** 用户管理（iam-service）。 */
  readonly users: UsersApi;
  /** 角色管理（iam-service）。 */
  readonly roles: RolesApi;
  /** 权限管理（iam-service）。 */
  readonly permissions: PermissionsApi;
  /** 组织架构（iam-service，部门树）。 */
  readonly orgUnits: OrgUnitsApi;
  /** token 签发 / 刷新 / 吊销 / JWKS / API Key（token-service）。 */
  readonly tokens: TokensApi;
}

/** 创建 IAM 服务客户端。 */
export function createIamServices(options: IamServicesOptions = {}): IamServices {
  const http = new HttpCore(options);
  return {
    auth: new AuthApi(http),
    users: new UsersApi(http),
    roles: new RolesApi(http),
    permissions: new PermissionsApi(http),
    orgUnits: new OrgUnitsApi(http),
    tokens: new TokensApi(http),
  };
}
