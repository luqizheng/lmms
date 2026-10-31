// 角色管理（iam-service，经网关 /v1/iam/roles*）。
// 路由事实源：services/iam-service/src/routes/roles.rs。

import type { HttpCore } from "./http.js";
import type {
  CreateRoleRequest,
  RoleListResponse,
  RoleQuery,
  RoleResponse,
  UpdateRoleRequest,
} from "./types.js";

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

  /** 创建角色：POST /v1/iam/roles。角色 code 重复返回 409 "duplicate role"。 */
  create(request: CreateRoleRequest): Promise<RoleResponse> {
    return this.http.post<RoleResponse>("/v1/iam/roles", request, { auth: true });
  }

  /** 更新角色：PUT /v1/iam/roles/{role_id}。 */
  update(roleId: string, request: UpdateRoleRequest): Promise<RoleResponse> {
    return this.http.put<RoleResponse>(`/v1/iam/roles/${encodeURIComponent(roleId)}`, request, { auth: true });
  }

  /** 删除角色：DELETE /v1/iam/roles/{role_id}，返回 204。 */
  remove(roleId: string): Promise<void> {
    return this.http.delete<void>(`/v1/iam/roles/${encodeURIComponent(roleId)}`, { auth: true });
  }
}
