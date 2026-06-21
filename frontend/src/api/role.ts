import { http } from "@/utils/http";

export interface PageResult<T> {
  items: T[];
  total: number;
  page: number;
  size: number;
}

export interface Role {
  id: number;
  name: string;
  code: string;
  description: string;
  status: number;
  created_at: string;
  updated_at: string;
}

export function listRoles(params: {
  page?: number;
  size?: number;
  keyword?: string;
}) {
  return http.request<PageResult<Role>>("get", "/v1/iam/roles", { params });
}

export function createRole(data: Pick<Role, "name" | "code" | "description">) {
  return http.request<Role>("post", "/v1/iam/roles", { data });
}

export function getRole(id: number) {
  return http.request<Role>("get", `/v1/iam/roles/${id}`);
}

export function updateRole(
  id: number,
  data: Pick<Role, "name" | "code" | "description" | "status">
) {
  return http.request<Role>("put", `/v1/iam/roles/${id}`, { data });
}

export function deleteRole(id: number) {
  return http.request<void>("delete", `/v1/iam/roles/${id}`);
}

export function assignRolePermissions(id: number, permissionIds: number[]) {
  return http.request<void>("post", `/v1/iam/roles/${id}/permissions`, {
    data: { permission_ids: permissionIds }
  });
}
