import { http } from "@/utils/http";

// ============== 用户 ==============
export interface IamUser {
  id: string;
  username: string;
  email?: string;
  phone?: string;
  nickname: string;
  avatar?: string;
  status: string;
  email_verified: boolean;
  phone_verified: boolean;
  last_login_at?: string;
  last_login_ip?: string;
  created_at: string;
  updated_at: string;
}

export interface IamUserQuery {
  keyword?: string;
  status?: string;
  org_unit_id?: string;
  page: number;
  size: number;
}

export interface IamUserListResponse {
  items: IamUser[];
  total: number;
  page: number;
  size: number;
}

export interface CreateIamUserRequest {
  username: string;
  password: string;
  email?: string;
  phone?: string;
  nickname: string;
  avatar?: string;
  role_ids?: string[];
}

export interface UpdateIamUserRequest {
  nickname?: string;
  email?: string;
  phone?: string;
  avatar?: string;
  status?: string;
}

export function listIamUsers(params: IamUserQuery) {
  return http.request<IamUserListResponse>("get", "/iam/users", { params });
}

export function createIamUser(data: CreateIamUserRequest) {
  return http.request<IamUser>("post", "/iam/users", { data });
}

export function getIamUser(id: string) {
  return http.request<IamUser>("get", `/iam/users/${encodeURIComponent(id)}`);
}

export function updateIamUser(id: string, data: UpdateIamUserRequest) {
  return http.request<IamUser>("put", `/iam/users/${encodeURIComponent(id)}`, {
    data
  });
}

export function deleteIamUser(id: string) {
  return http.request<void>("delete", `/iam/users/${encodeURIComponent(id)}`);
}

// ============== 角色 ==============
export interface IamRole {
  id: string;
  name: string;
  code: string;
  description?: string;
  data_scope: string;
  status: string;
  created_at: string;
  updated_at: string;
}

export interface IamRoleQuery {
  keyword?: string;
  status?: string;
  page: number;
  size: number;
}

export interface IamRoleListResponse {
  items: IamRole[];
  total: number;
  page: number;
  size: number;
}

export interface CreateIamRoleRequest {
  name: string;
  code: string;
  description?: string;
  data_scope: number;
}

export interface UpdateIamRoleRequest {
  name?: string;
  description?: string;
  data_scope?: number;
  status?: string;
}

export function listIamRoles(params: IamRoleQuery) {
  return http.request<IamRoleListResponse>("get", "/iam/roles", { params });
}

export function createIamRole(data: CreateIamRoleRequest) {
  return http.request<IamRole>("post", "/iam/roles", { data });
}

export function updateIamRole(id: string, data: UpdateIamRoleRequest) {
  return http.request<IamRole>("put", `/iam/roles/${encodeURIComponent(id)}`, {
    data
  });
}

export function deleteIamRole(id: string) {
  return http.request<void>("delete", `/iam/roles/${encodeURIComponent(id)}`);
}

export function assignRolePermissions(id: string, permission_ids: string[]) {
  return http.request<void>(
    "put",
    `/iam/roles/${encodeURIComponent(id)}/permissions`,
    { data: { permission_ids } }
  );
}

// ============== 权限 ==============
export interface IamPermission {
  id: string;
  code: string;
  name: string;
  description?: string;
  category: string;
  resource?: string;
  action?: string;
  sort_order: number;
  status: string;
  created_at: string;
  updated_at: string;
}

export interface IamPermissionQuery {
  keyword?: string;
  category?: number;
  page: number;
  size: number;
}

export interface IamPermissionListResponse {
  items: IamPermission[];
  total: number;
  page: number;
  size: number;
}

export interface CreateIamPermissionRequest {
  code: string;
  name: string;
  description?: string;
  category: number;
  resource?: string;
  action?: string;
  sort_order?: number;
}

export interface UpdateIamPermissionRequest {
  name?: string;
  description?: string;
  sort_order?: number;
  status?: string;
}

export function listIamPermissions(params: IamPermissionQuery) {
  return http.request<IamPermissionListResponse>("get", "/iam/permissions", {
    params
  });
}

export function createIamPermission(data: CreateIamPermissionRequest) {
  return http.request<IamPermission>("post", "/iam/permissions", { data });
}

export function updateIamPermission(
  id: string,
  data: UpdateIamPermissionRequest
) {
  return http.request<IamPermission>(
    "put",
    `/iam/permissions/${encodeURIComponent(id)}`,
    { data }
  );
}

export function deleteIamPermission(id: string) {
  return http.request<void>(
    "delete",
    `/iam/permissions/${encodeURIComponent(id)}`
  );
}

// ============== 菜单 ==============
export interface IamMenu {
  id: string;
  parent_id?: string;
  name: string;
  code: string;
  path: string;
  component?: string;
  icon?: string;
  sort_order: number;
  visible: boolean;
  status: string;
  permission_id?: string;
  created_at: string;
  updated_at: string;
  children: IamMenu[];
}

export interface IamMenuListResponse {
  items: IamMenu[];
  total: number;
}

export interface CreateIamMenuRequest {
  parent_id?: string;
  name: string;
  code: string;
  path: string;
  component?: string;
  icon?: string;
  sort_order?: number;
  visible?: boolean;
  permission_id?: string;
}

export interface UpdateIamMenuRequest {
  parent_id?: string;
  name?: string;
  code?: string;
  path?: string;
  component?: string;
  icon?: string;
  sort_order?: number;
  visible?: boolean;
  permission_id?: string;
  status?: string;
}

export function listIamMenus() {
  return http.request<IamMenuListResponse>("get", "/iam/menus");
}

export function createIamMenu(data: CreateIamMenuRequest) {
  return http.request<IamMenu>("post", "/iam/menus", { data });
}

export function updateIamMenu(id: string, data: UpdateIamMenuRequest) {
  return http.request<IamMenu>("put", `/iam/menus/${encodeURIComponent(id)}`, {
    data
  });
}

export function deleteIamMenu(id: string) {
  return http.request<void>("delete", `/iam/menus/${encodeURIComponent(id)}`);
}

// ============== 组织（部门） ==============
export interface IamOrgUnit {
  id: string;
  parent_id?: string;
  name: string;
  code: string;
  level: number;
  sort_order: number;
  status: string;
  created_at: string;
  updated_at: string;
  children: IamOrgUnit[];
}

export interface CreateIamOrgUnitRequest {
  parent_id?: string;
  name: string;
  code: string;
  sort_order?: number;
}

export interface UpdateIamOrgUnitRequest {
  parent_id?: string;
  name?: string;
  code?: string;
  sort_order?: number;
  status?: string;
}

export function listIamOrgUnits() {
  return http.request<{ items: IamOrgUnit[] }>("get", "/iam/org-units");
}

export function createIamOrgUnit(data: CreateIamOrgUnitRequest) {
  return http.request<IamOrgUnit>("post", "/iam/org-units", { data });
}

export function updateIamOrgUnit(id: string, data: UpdateIamOrgUnitRequest) {
  return http.request<IamOrgUnit>(
    "put",
    `/iam/org-units/${encodeURIComponent(id)}`,
    { data }
  );
}

export function deleteIamOrgUnit(id: string) {
  return http.request<void>(
    "delete",
    `/iam/org-units/${encodeURIComponent(id)}`
  );
}
