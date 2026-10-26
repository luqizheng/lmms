import { http } from "./client";
import type { LoginResult } from "./client";

export interface LoginRequest {
  username: string;
  password: string;
}

export interface SetupStatus {
  setup_required: boolean;
}

/** 菜单树（来自 DB /v1/iam/menus） */
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
  children: IamMenu[];
}

export interface IamMenuListResponse {
  items: IamMenu[];
  total: number;
}

export function login(data: LoginRequest) {
  return http.post<LoginResult>("/v1/iam/auth/login", data);
}

export function logout() {
  return http.post<void>("/v1/iam/auth/logout");
}

export function getSetupStatus() {
  return http.get<SetupStatus>("/v1/iam/auth/setup-status");
}

export function listMenus() {
  return http.get<IamMenuListResponse>("/v1/iam/menus");
}

export { translateError } from "./client";
export type { LoginResult } from "./client";
