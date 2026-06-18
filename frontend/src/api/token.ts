import { http } from "@/utils/http";

export interface AccessTokenRequest {
  grant_type: string;
  code: string;
  client_id: string;
  client_secret: string;
}

export interface AccessTokenResult {
  access_token: string;
  refresh_token: string;
  token_type: string;
  expires_in: number;
}

export function createAccessToken(data: AccessTokenRequest) {
  return http.request<AccessTokenResult>("post", "/v1/tokens/access", {
    data
  });
}

export interface RefreshTokenRequest {
  refresh_token: string;
  client_id: string;
  client_secret: string;
}

export function refreshToken(data: RefreshTokenRequest) {
  return http.request<AccessTokenResult>("post", "/v1/tokens/refresh", {
    data
  });
}

export function revokeToken(token: string) {
  return http.request<void>("post", "/v1/tokens/revoke", {
    data: { token }
  });
}

export function listResources() {
  return http.request<string[]>("get", "/v1/tokens/resource");
}

export function createApiKey(data: { name: string; scopes?: string[] }) {
  return http.request<{ api_key: string; id: string }>(
    "post",
    "/v1/tokens/api_key",
    { data }
  );
}

// ========== OAuth 客户端管理 ==========

export interface OAuthClient {
  client_id: string;
  name: string;
  permissions: string[];
  grant_types: string[];
  access_token_ttl_sec: number;
  refresh_token_ttl_sec: number;
  status: string;
  created_at: string;
  updated_at: string;
}

export interface OAuthClientListQuery {
  keyword?: string;
  status?: string;
  page?: number;
  size?: number;
}

export interface OAuthClientListResult {
  list: OAuthClient[];
  total: number;
  page: number;
  size: number;
}

export interface CreateOAuthClientRequest {
  name: string;
  permissions: string[];
  grant_types: string[];
  access_token_ttl_sec: number;
  refresh_token_ttl_sec: number;
  status: string;
}

export interface CreateOAuthClientResult {
  client_id: string;
  client_secret: string;
  client: OAuthClient;
}

export interface UpdateOAuthClientRequest {
  name?: string;
  permissions?: string[];
  grant_types?: string[];
  access_token_ttl_sec?: number;
  refresh_token_ttl_sec?: number;
  status?: string;
}

export interface ResetSecretResult {
  client_id: string;
  client_secret: string;
}

export function listOAuthClients(params?: OAuthClientListQuery) {
  return http.request<OAuthClientListResult>(
    "get",
    "/api/v1/admin/oauth-clients",
    { params }
  );
}

export function createOAuthClient(data: CreateOAuthClientRequest) {
  return http.request<CreateOAuthClientResult>(
    "post",
    "/api/v1/admin/oauth-clients",
    { data }
  );
}

export function updateOAuthClient(
  client_id: string,
  data: UpdateOAuthClientRequest
) {
  return http.request<OAuthClient>(
    "put",
    `/api/v1/admin/oauth-clients/${encodeURIComponent(client_id)}`,
    { data }
  );
}

export function deleteOAuthClient(client_id: string) {
  return http.request<void>(
    "delete",
    `/api/v1/admin/oauth-clients/${encodeURIComponent(client_id)}`
  );
}

export function resetOAuthClientSecret(client_id: string) {
  return http.request<ResetSecretResult>(
    "post",
    `/api/v1/admin/oauth-clients/${encodeURIComponent(client_id)}/reset-secret`
  );
}
