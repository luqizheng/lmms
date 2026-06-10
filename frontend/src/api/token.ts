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
  return http.request<AccessTokenResult>("post", "/api/v1/tokens/access", { data });
}

export interface RefreshTokenRequest {
  refresh_token: string;
  client_id: string;
  client_secret: string;
}

export function refreshToken(data: RefreshTokenRequest) {
  return http.request<AccessTokenResult>("post", "/api/v1/tokens/refresh", { data });
}

export function revokeToken(token: string) {
  return http.request<void>("post", "/api/v1/tokens/revoke", { data: { token } });
}

export function listResources() {
  return http.request<string[]>("get", "/api/v1/tokens/resource");
}

export function createApiKey(data: { name: string; scopes?: string[] }) {
  return http.request<{ api_key: string; id: string }>(
    "post",
    "/api/v1/tokens/api_key",
    { data }
  );
}
