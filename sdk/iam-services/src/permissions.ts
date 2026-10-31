// 权限管理（iam-service，经网关 /v1/iam/permissions*）。
// 路由事实源：services/iam-service/src/routes/permissions.rs。

import type { HttpCore } from "./http.js";
import type {
  CreatePermissionRequest,
  PermissionListResponse,
  PermissionQuery,
  PermissionResponse,
  UpdatePermissionRequest,
} from "./types.js";

export class PermissionsApi {
  constructor(private readonly http: HttpCore) {}

  /** 分页查询权限：GET /v1/iam/permissions。 */
  page(query: PermissionQuery = {}): Promise<PermissionListResponse> {
    return this.http.get<PermissionListResponse>("/v1/iam/permissions", {
      params: query,
      auth: true,
    });
  }

  /** 查询单个权限：GET /v1/iam/permissions/{permission_id}。 */
  get(permissionId: string): Promise<PermissionResponse> {
    return this.http.get<PermissionResponse>(
      `/v1/iam/permissions/${encodeURIComponent(permissionId)}`,
      { auth: true }
    );
  }

  /** 创建权限：POST /v1/iam/permissions。code 重复返回 409。 */
  create(request: CreatePermissionRequest): Promise<PermissionResponse> {
    return this.http.post<PermissionResponse>("/v1/iam/permissions", request, {
      auth: true,
    });
  }

  /** 更新权限：PUT /v1/iam/permissions/{permission_id}。 */
  update(
    permissionId: string,
    request: UpdatePermissionRequest
  ): Promise<PermissionResponse> {
    return this.http.put<PermissionResponse>(
      `/v1/iam/permissions/${encodeURIComponent(permissionId)}`,
      request,
      { auth: true }
    );
  }

  /** 删除权限：DELETE /v1/iam/permissions/{permission_id}，返回 204。 */
  remove(permissionId: string): Promise<void> {
    return this.http.delete<void>(
      `/v1/iam/permissions/${encodeURIComponent(permissionId)}`,
      { auth: true }
    );
  }
}
