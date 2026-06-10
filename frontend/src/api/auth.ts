import { http } from "@/utils/http";

export interface LoginRequest {
  username: string;
  password: string;
}

export interface LoginResult {
  code: string;
  expires_in: number;
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
  email?: string;
  phone?: string;
  nickname?: string;
}

export interface RegisterResult {
  id: number;
  username: string;
}

export function register(data: RegisterRequest) {
  return http.request<RegisterResult>("post", "/api/v1/auth/register", { data });
}
