// iam-service / token-service DTO 镜像。字段名以后端 serde 序列化为唯一事实源：
//   - services/iam-service/src/dto/{auth,user,role}.rs
//   - services/token-service/openapi.yaml
//   - infrastructure/config/api-gateway/routes/*.router.toml（/v1 公开路径）
// 后端对成功响应不包 {code,message,data} 信封，DTO 即响应体本身。

// ---------- 认证（auth） ----------

/** 登录/刷新响应中的用户信息（含角色编码与权限编码）。 */
export interface UserInfo {
  id: string;
  username: string;
  nickname: string;
  email: string | null;
  phone: string | null;
  avatar: string | null;
  roles: string[];
  permissions: string[];
}

export interface LoginRequest {
  username: string;
  password: string;
  /** 可选租户标识，缺省由服务端取默认租户。 */
  tenant_slug?: string;
}

/**
 * 登录与刷新共用的响应。iam 登录链路的 refresh token 不轮换：
 * refresh 响应中的 refresh_token 与传入值相同，调用方不应覆盖已存储的值。
 */
export interface LoginResponse {
  access_token: string;
  refresh_token: string;
  token_type: string;
  /** access token 有效期（秒），服务端默认 900。 */
  expires_in: number;
  user: UserInfo;
}

export interface ChangePasswordRequest {
  old_password: string;
  /** 最少 8 位。 */
  new_password: string;
}

export interface SetupStatus {
  setup_required: boolean;
  user_count: number;
}

/** 登录会话（GET /v1/iam/auth/sessions）。 */
export interface SessionInfo {
  id: string;
  user_id: string;
  client_ip: string;
  user_agent: string | null;
  created_at: string;
  expires_at: string;
  is_current: boolean;
}

// ---------- 用户（users） ----------

export interface UserResponse {
  id: string;
  username: string;
  email: string | null;
  phone: string | null;
  nickname: string;
  avatar: string | null;
  status: string;
  email_verified: boolean;
  phone_verified: boolean;
  last_login_at: string | null;
  last_login_ip: string | null;
  created_at: string;
  updated_at: string;
}

export interface UserListResponse {
  items: UserResponse[];
  total: number;
  page: number;
  size: number;
}

/** 轻量用户（昵称展示用，GET /v1/iam/users/simple）。 */
export interface UserSimpleResponse {
  id: string;
  nickname: string;
}

export interface UserSimpleListResponse {
  items: UserSimpleResponse[];
  total: number;
  page: number;
  size: number;
}

export interface UserQuery {
  keyword?: string;
  status?: string;
  org_unit_id?: string;
  page?: number;
  size?: number;
}

export interface UserSimpleQuery {
  keyword?: string;
  page?: number;
  size?: number;
}

export interface CreateUserRequest {
  username: string;
  /** 最少 8 位。 */
  password: string;
  nickname: string;
  email?: string;
  /** 11 位手机号（服务端校验）。 */
  phone?: string;
  avatar?: string;
  role_ids?: string[];
}

export interface UpdateUserRequest {
  nickname?: string;
  email?: string;
  phone?: string;
  avatar?: string;
  status?: string;
}

/** PUT /v1/iam/users/{user_id}/roles 的响应。 */
export interface AssignUserRolesResponse {
  user_id: string;
  role_ids: string[];
}

// ---------- 角色（roles，v1 只读） ----------

export interface RoleResponse {
  id: string;
  name: string;
  code: string;
  description: string | null;
  data_scope: string;
  status: string;
  created_at: string;
  updated_at: string;
}

export interface RoleListResponse {
  items: RoleResponse[];
  total: number;
  page: number;
  size: number;
}

export interface RoleQuery {
  keyword?: string;
  status?: string;
  page?: number;
  size?: number;
}

export interface CreateRoleRequest {
  name: string;
  code: string;
  description?: string;
  /** 服务端 DTO 为必填 i32；响应中 data_scope 是字符串（不对称，照实镜像） */
  data_scope: number;
}

export interface UpdateRoleRequest {
  name?: string;
  description?: string;
  data_scope?: number;
  status?: string;
}

// ---------- 权限 ----------

export interface PermissionResponse {
  id: string;
  code: string;
  name: string;
  description: string | null;
  /** 出参为字符串（创建入参 category 是数字，不对称照实镜像） */
  category: string;
  resource: string | null;
  action: string | null;
  sort_order: number;
  status: string;
  created_at: string;
  updated_at: string;
}

export interface PermissionListResponse {
  items: PermissionResponse[];
  total: number;
  page: number;
  size: number;
}

export interface PermissionQuery {
  keyword?: string;
  category?: number;
  page?: number;
  size?: number;
}

export interface CreatePermissionRequest {
  code: string;
  name: string;
  description?: string;
  /** 服务端 DTO 为必填 i32；响应中 category 是字符串（不对称照实镜像） */
  category: number;
  resource?: string;
  action?: string;
  sort_order?: number;
}

export interface UpdatePermissionRequest {
  name?: string;
  description?: string;
  sort_order?: number;
  status?: string;
}

// ---------- 组织架构 ----------

export interface OrgUnitResponse {
  id: string;
  parent_id: string | null;
  name: string;
  code: string;
  level: number;
  sort_order: number;
  status: string;
  created_at: string;
  updated_at: string;
  /** GET /v1/iam/org-units 返回部门树（根节点数组，children 递归） */
  children: OrgUnitResponse[];
}

export interface CreateOrgUnitRequest {
  parent_id?: string | null;
  name: string;
  code: string;
  sort_order?: number;
}

export interface UpdateOrgUnitRequest {
  parent_id?: string | null;
  name?: string;
  code?: string;
  sort_order?: number;
  status?: string;
}

// ---------- token（token-service，经网关 /v1/tokens、/v1/api-keys） ----------

export type TokenGrantType = "client_credentials" | "authorization_code";

/** 机器/应用身份取 token：client_credentials 或 authorization_code 换 access token。 */
export type TokenAccessRequest =
  | {
      grant_type: "client_credentials";
      client_id: string;
      client_secret: string;
    }
  | {
      grant_type: "authorization_code";
      client_id: string;
      client_secret: string;
      code: string;
    };

/**
 * OAuth 刷新请求（token-service 链路，refresh token 单次使用轮换 + 重放检测）。
 * 注意：与 iam 登录链路（AuthApi.refresh）不同，那条链路不轮换。
 */
export interface TokenRefreshRequest {
  refresh_token: string;
  client_id: string;
  client_secret: string;
}

export interface TokenResponse {
  access_token: string;
  refresh_token: string;
  token_type: string;
  expires_in: number;
}

/** 按 jti 将 access token 拉黑（POST /v1/tokens/revoke）。 */
export interface TokenRevokeRequest {
  token_jti: string;
  /** 拉黑有效期（秒），建议取该 token 剩余寿命。 */
  expires_in: number;
}

export interface TokenRevokeResponse {
  success: boolean;
}

/** 标准 JWKS（GET /v1/tokens/.well-known/jwks.json，RS256）。 */
export interface Jwks {
  keys: Record<string, unknown>[];
}

export interface ApiKeySummary {
  id: string;
  name: string;
  permissions: string[];
  status: string;
  created_at: string;
}

export interface ApiKeyListResponse {
  items: ApiKeySummary[];
  total: number;
  page: number;
  size: number;
}

/** 创建 API Key（client_secret / api_key_secret 仅在创建响应中返回一次）。 */
export interface CreateApiKeyRequest {
  name: string;
  permissions?: string[];
  grant_types?: string[];
  access_token_ttl_sec?: number;
  refresh_token_ttl_sec?: number;
}

export interface ApiKeyCreated {
  api_key_id: string;
  api_key_secret: string;
}

// ── Service JWT audience grants（token-service web 管理入口）──

/** 一条 owner → audience 签发授权关系（无任何凭据材料）。 */
export interface ServiceJwtGrant {
  owner: string;
  audience: string;
  /** "active" | "disabled" */
  status: string;
  /** true = 内置系统授权（服务端保护：改删返回 409）。 */
  system_managed?: boolean;
  created_at?: string;
  updated_at?: string;
}

/** grant 分页查询结果：{ items, total, page, size }。 */
export interface ServiceJwtGrantListResponse {
  items: ServiceJwtGrant[];
  total: number;
  page: number;
  size: number;
}

/** grant 列表查询参数（owner / audience 精确过滤 + status + 分页）。 */
export interface ServiceJwtGrantListQuery {
  owner?: string;
  audience?: string;
  /** "active" | "disabled"；其他值服务端返回 400。 */
  status?: string;
  /** 页码，从 1 起。 */
  page?: number;
  /** 每页条数（1..=100）。 */
  size?: number;
}

/** 创建 grant 请求体。 */
export interface CreateServiceJwtGrantRequest {
  owner: string;
  audience: string;
}

/** 启停 grant 请求体。 */
export interface SetServiceJwtGrantStatusRequest {
  /** "active" | "disabled" */
  status: string;
}
