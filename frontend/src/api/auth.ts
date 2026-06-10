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
  return http.request<LoginResult>("post", "/auth/login", { data });
}

export function logout() {
  return http.request<void>("post", "/auth/logout");
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
  return http.request<RegisterResult>("post", "/auth/register", { data });
}
