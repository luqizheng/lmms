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
  return http.request<AccessTokenResult>("post", "/tokens/access", { data });
}

export interface RefreshTokenRequest {
  refresh_token: string;
  client_id: string;
  client_secret: string;
}

export function refreshToken(data: RefreshTokenRequest) {
  return http.request<AccessTokenResult>("post", "/tokens/refresh", { data });
}

export function revokeToken(token: string) {
  return http.request<void>("post", "/tokens/revoke", { data: { token } });
}

export function listResources() {
  return http.request<string[]>("get", "/tokens/resource");
}

export function createApiKey(data: { name: string; scopes?: string[] }) {
  return http.request<{ api_key: string; id: string }>("post", "/tokens/api_key", {
    data
  });
}
