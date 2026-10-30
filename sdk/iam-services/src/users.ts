// 用户管理（iam-service，经网关 /v1/iam/users*）。全部端点需要鉴权。
// 路由事实源：services/iam-service/src/routes/users.rs（角色分配在 routes/roles.rs）。

import type { HttpCore } from "./http.js";
import type {
  AssignUserRolesResponse,
  CreateUserRequest,
  UpdateUserRequest,
  UserListResponse,
  UserQuery,
  UserResponse,
  UserSimpleListResponse,
  UserSimpleQuery,
} from "./types.js";

export class UsersApi {
  constructor(private readonly http: HttpCore) {}

  /** 分页查询用户：GET /v1/iam/users。 */
  page(query: UserQuery = {}): Promise<UserListResponse> {
    return this.http.get<UserListResponse>("/v1/iam/users", { params: query, auth: true });
  }

  /** 分页查询轻量用户（id + nickname）：GET /v1/iam/users/simple。 */
  pageSimple(query: UserSimpleQuery = {}): Promise<UserSimpleListResponse> {
    return this.http.get<UserSimpleListResponse>("/v1/iam/users/simple", { params: query, auth: true });
  }

  /** 查询单个用户：GET /v1/iam/users/{user_id}。 */
  get(userId: string): Promise<UserResponse> {
    return this.http.get<UserResponse>(`/v1/iam/users/${encodeURIComponent(userId)}`, { auth: true });
  }

  /** 创建用户：POST /v1/iam/users。 */
  create(request: CreateUserRequest): Promise<UserResponse> {
    return this.http.post<UserResponse>("/v1/iam/users", request, { auth: true });
  }

  /** 更新用户：PUT /v1/iam/users/{user_id}。 */
  update(userId: string, request: UpdateUserRequest): Promise<UserResponse> {
    return this.http.put<UserResponse>(`/v1/iam/users/${encodeURIComponent(userId)}`, request, { auth: true });
  }

  /** 删除用户：DELETE /v1/iam/users/{user_id}，返回 204。 */
  remove(userId: string): Promise<void> {
    return this.http.delete<void>(`/v1/iam/users/${encodeURIComponent(userId)}`, { auth: true });
  }

  /** 分配角色：PUT /v1/iam/users/{user_id}/roles，请求体 {role_ids: string[]}。 */
  assignRoles(userId: string, roleIds: string[]): Promise<AssignUserRolesResponse> {
    return this.http.put<AssignUserRolesResponse>(`/v1/iam/users/${encodeURIComponent(userId)}/roles`, { role_ids: roleIds }, { auth: true });
  }
}
