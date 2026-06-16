import { http } from "@/utils/http";

export interface LoginRequest {
  username: string;
  password: string;
}

export interface LoginUser {
  id: string;
  username: string;
  nickname: string;
  email?: string | null;
  phone?: string | null;
  avatar?: string | null;
  roles: string[];
  permissions: string[];
}

export interface LoginResult {
  access_token: string;
  refresh_token: string;
  token_type: string;
  expires_in: number;
  user: LoginUser;
}

export function login(data: LoginRequest) {
  return http.request<LoginResult>("post", "/api/v1/auth/login", { data });
}

export function logout() {
  return http.request<void>("post", "/api/v1/auth/logout");
}

export function getSetupStatus() {
  return http.request<{ setup_required: boolean }>(
    "get",
    "/api/v1/auth/setup-status"
  );
}

export interface RegisterRequest {
  username: string;
  password: string;
  nickname: string;
  email?: string;
  phone?: string;
}

export interface RegisterResult {
  id: number;
  username: string;
}

export function register(data: RegisterRequest) {
  return http.request<RegisterResult>("post", "/api/v1/auth/register", { data });
}
