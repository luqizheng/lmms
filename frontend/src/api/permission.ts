import { http } from "@/utils/http";

export interface Permission {
  id: number;
  code: string;
  name: string;
  resource: string;
  action: string;
}

export function listPermissions() {
  return http.request<Permission[]>("get", "/permissions");
}
