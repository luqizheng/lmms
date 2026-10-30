// 角色查询（iam-service，经网关 /v1/iam/roles*）。v1 只读，供用户-角色分配等管理界面取选项。
// 路由事实源：services/iam-service/src/routes/roles.rs。

import type { HttpCore } from "./http.js";
import type { RoleListResponse, RoleQuery, RoleResponse } from "./types.js";

export class RolesApi {
  constructor(private readonly http: HttpCore) {}

  /** 分页查询角色：GET /v1/iam/roles。 */
  list(query: RoleQuery = {}): Promise<RoleListResponse> {
    return this.http.get<RoleListResponse>("/v1/iam/roles", { params: query, auth: true });
  }

  /** 查询单个角色：GET /v1/iam/roles/{role_id}。 */
  get(roleId: string): Promise<RoleResponse> {
    return this.http.get<RoleResponse>(`/v1/iam/roles/${encodeURIComponent(roleId)}`, { auth: true });
  }
}
