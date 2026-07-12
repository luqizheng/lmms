import { http } from "@/utils/http";

import type {
  ConfigItem,
  CreateConfigItem,
  UpdateConfigItem,
  Namespace,
  PagedConfigItems,
  CreateNamespace,
  UpdateNamespace
} from "./config/config.types";

export type {
  ConfigItem,
  CreateConfigItem,
  UpdateConfigItem,
  Namespace,
  PagedConfigItems,
  CreateNamespace,
  UpdateNamespace
};

// ============== 配置项 ==============
export interface QueryConfigItemsParams {
  namespace_id?: string;
  key?: string;
  page?: number;
  size?: number;
}

export function listConfigItems(params?: QueryConfigItemsParams) {
  return http.request<PagedConfigItems>("get", "/v1/config/item", { params });
}

export function getConfigItem(id: string) {
  return http.request<ConfigItem>(
    "get",
    `/v1/config/item/${encodeURIComponent(id)}`
  );
}

export function createConfigItem(body: CreateConfigItem) {
  return http.request<ConfigItem>("post", "/v1/config/item", { data: body });
}

export function updateConfigItem(id: string, body: UpdateConfigItem) {
  return http.request<ConfigItem>(
    "put",
    `/v1/config/item/${encodeURIComponent(id)}`,
    { data: body }
  );
}

export function deleteConfigItem(id: string) {
  return http.request<{ success: boolean }>(
    "delete",
    `/v1/config/item/${encodeURIComponent(id)}`
  );
}

export function exportConfig(namespaceId: string) {
  return http.request<string>(
    "get",
    `/v1/config/export/${encodeURIComponent(namespaceId)}`
  );
}

export function importConfig(namespaceId: string, body: string) {
  return http.request<{ success: boolean; count: number }>(
    "post",
    `/v1/config/import/${encodeURIComponent(namespaceId)}`,
    { data: body }
  );
}

// ============== 命名空间 ==============
export interface QueryNamespacesParams {
  app?: string;
  env?: string;
  page?: number;
  size?: number;
}

export function listNamespaces(params?: QueryNamespacesParams) {
  return http.request<{ items: Namespace[]; total: number }>(
    "get",
    "/v1/config/namespace",
    { params }
  );
}

export function getNamespace(id: string) {
  return http.request<Namespace>(
    "get",
    `/v1/config/namespace/${encodeURIComponent(id)}`
  );
}

export function createNamespace(body: CreateNamespace) {
  return http.request<Namespace>("post", "/v1/config/namespace", {
    data: body
  });
}

export function updateNamespace(id: string, body: UpdateNamespace) {
  return http.request<Namespace>(
    "put",
    `/v1/config/namespace/${encodeURIComponent(id)}`,
    { data: body }
  );
}

export function deleteNamespace(id: string) {
  return http.request<{ success: boolean }>(
    "delete",
    `/v1/config/namespace/${encodeURIComponent(id)}`
  );
}
