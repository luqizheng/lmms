// Auto-generated types from OpenAPI spec
// DO NOT EDIT

export interface CreatePresignUrl {
  expires_in_seconds?: number;
  file_id: string;
  operation: string;
}

export interface FileInfo {
  content_type?: string;
  created_at: string;
  expires_at?: string;
  filename: string;
  hash?: string;
  id: string;
  is_temporary: boolean;
  size: number;
  storage_path: string;
  tenant_id: number;
  updated_at: string;
  url?: string;
  user_id?: string;
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

export interface PagedFiles {
  items: FileInfo[];
  page: number;
  size: number;
  total: number;
}

export interface PresignUrlResponse {
  expires_at: string;
  operation: string;
  url: string;
}

export interface UploadFileMeta {
  content_type?: string;
  expires_in_seconds?: number;
  filename: string;
  is_temporary?: boolean;
  tenant_id?: number;
  user_id?: string;
}

export interface ApiResponse<T = any> {
  code: string;
  message: string;
  data?: T;
  request_id?: string;
}
