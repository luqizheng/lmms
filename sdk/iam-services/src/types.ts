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
