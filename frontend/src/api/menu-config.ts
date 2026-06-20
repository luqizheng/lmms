import { http } from "@/utils/http";
import type {
  MenuConfigDetail,
  MenuConfigListResponse
} from "@/views/system/menu/utils/types";

interface ApiResponse<T> {
  code: number;
  message: string;
  data: T;
}

interface CreateMenuConfigRequest {
  name: string;
  description?: string;
  routes_json: unknown[];
  created_by?: string;
}

interface UpdateMenuConfigRequest {
  name?: string;
  description?: string;
  routes_json?: unknown[];
}

export const listMenuConfigs = () => {
  return http.request<ApiResponse<MenuConfigListResponse>>(
    "get",
    "/v1/pure/menu-configs"
  );
};

export const createMenuConfig = (data: CreateMenuConfigRequest) => {
  return http.request<ApiResponse<MenuConfigDetail>>(
    "post",
    "/v1/pure/menu-configs",
    {
      data
    }
  );
};

export const getMenuConfig = (id: string) => {
  return http.request<ApiResponse<MenuConfigDetail>>(
    "get",
    `/v1/pure/menu-configs/${id}`
  );
};

export const updateMenuConfig = (id: string, data: UpdateMenuConfigRequest) => {
  return http.request<ApiResponse<MenuConfigDetail>>(
    "put",
    `/v1/pure/menu-configs/${id}`,
    { data }
  );
};

export const publishMenuConfig = (id: string) => {
  return http.request<ApiResponse<MenuConfigDetail>>(
    "post",
    `/v1/pure/menu-configs/${id}/publish`
  );
};

export const cloneMenuConfig = (id: string) => {
  return http.request<ApiResponse<MenuConfigDetail>>(
    "post",
    `/v1/pure/menu-configs/${id}/clone`
  );
};

export const deleteMenuConfig = (id: string) => {
  return http.request<void>("delete", `/v1/pure/menu-configs/${id}`);
};
