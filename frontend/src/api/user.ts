import { http } from "@/utils/http";

export interface User {
  id: number;
  username: string;
  email: string;
  nickname: string;
  avatar: string;
  status: number;
  roles: string[];
  created_at: string;
  updated_at: string;
}

export interface ListUsersQuery {
  page?: number;
  size?: number;
  keyword?: string;
  status?: number;
}

export interface PageResult<T> {
  list: T[];
  total: number;
  page: number;
  size: number;
}

export function getCurrentUser() {
  return http.request<User>("get", "/users/me");
}

export function updateCurrentUser(data: Partial<User>) {
  return http.request<User>("put", "/users/me", { data });
}

export function listUsers(params: ListUsersQuery) {
  return http.request<PageResult<User>>("get", "/users", { params });
}

export function getUser(id: number) {
  return http.request<User>("get", `/users/${id}`);
}

export function patchUserExtends(id: number, data: Record<string, unknown>) {
  return http.request<User>("patch", `/users/${id}/extends`, { data });
}

export function assignUserRoles(id: number, roleIds: number[]) {
  return http.request<void>("post", `/users/${id}/roles`, {
    data: { role_ids: roleIds }
  });
}

export function removeUserRoles(id: number, roleIds: number[]) {
  return http.request<void>("delete", `/users/${id}/roles`, {
    data: { role_ids: roleIds }
  });
}
