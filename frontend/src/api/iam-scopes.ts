import { http } from "@/utils/http";

export interface ScopeItem {
  id: number;
  name: string;
}

export function listDepts() {
  return http.request<unknown>("get", "/v1/iam/dept/list");
}
export function listRoles() {
  return http.request<unknown[]>("get", "/v1/iam/roles");
}
export function listUsers() {
  return http.request<unknown>("get", "/v1/iam/users");
}
