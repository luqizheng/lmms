// Auto-generated types from OpenAPI spec
// DO NOT EDIT

export interface ConfigItem {
  created_at: string;
  description?: string;
  id: string;
  is_secret: boolean;
  key: string;
  namespace_id: string;
  updated_at: string;
  value: string;
  value_masked?: boolean;
  value_type: string;
}

export interface CreateConfigItem {
  description?: string;
  is_secret?: boolean;
  key: string;
  namespace_id: string;
  value: string;
  value_type?: string;
}

export interface CreateNamespace {
  app: string;
  description?: string;
  env: string;
  profile?: string;
}

export interface MigrationInfo {
  name: string;
  version: string;
}

export interface MigrationStateResponse {
  applied: string[];
  available: MigrationInfo[];
  current_version?: string;
  dialect: string;
  extra: string[];
  in_sync: boolean;
  missing: string[];
  service: string;
}

export interface Namespace {
  app: string;
  created_at: string;
  description?: string;
  env: string;
  id: string;
  profile?: string;
  tenant_id: number;
  updated_at: string;
}

export interface PagedConfigItems {
  items: ConfigItem[];
  page: number;
  size: number;
  total: number;
}

export interface PagedNamespaces {
  items: Namespace[];
  page: number;
  size: number;
  total: number;
}

export interface UpdateConfigItem {
  description?: string;
  is_secret?: boolean;
  value?: string;
  value_type?: string;
}

export interface UpdateNamespace {
  description?: string;
}

export interface ApiResponse<T = any> {
  code: string;
  message: string;
  data?: T;
  request_id?: string;
}
