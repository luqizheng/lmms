// Auto-generated types from OpenAPI spec
// DO NOT EDIT

export interface AccessRequest {
  client_id: string;
  client_secret: string;
  code: string;
  grant_type: string;
}

export interface AccessResponse {
  access_token: string;
  expires_in: number;
  refresh_token: string;
  token_type: string;
}

export interface ApiKeyCreated {
  api_key_id: string;
  api_key_secret: string;
}

export interface ApiKeySummary {
  created_at: string;
  id: string;
  name: string;
  permissions: string[];
  status: string;
}

export interface ClientDto {
  access_token_ttl_sec: number;
  client_id: string;
  created_at: string;
  grant_types: string[];
  name: string;
  permissions: string[];
  refresh_token_ttl_sec: number;
  status: string;
  updated_at: string;
}

export interface CreateApiKeyRequest {
  access_token_ttl_sec?: number;
  grant_types?: string[];
  name: string;
  permissions?: string[];
  refresh_token_ttl_sec?: number;
}

export interface CreateRequest {
  access_token_ttl_sec?: number;
  grant_types?: string[];
  name: string;
  permissions?: string[];
  refresh_token_ttl_sec?: number;
  status?: string;
}

export interface CreateResponse {
  client: ClientDto;
  client_id: string;
  client_secret: string;
}

export interface ListApiKeysResponse {
  items: ApiKeySummary[];
  page: number;
  size: number;
  total: number;
}

export interface ListResponse {
  list: ClientDto[];
  page: number;
  size: number;
  total: number;
}

export interface RefreshRequest {
  client_id: string;
  client_secret: string;
  refresh_token: string;
}

export interface RefreshResponse {
  access_token: string;
  expires_in: number;
  refresh_token: string;
  token_type: string;
}

export interface ResetSecretResponse {
  client_id: string;
  client_secret: string;
}

export interface RevokeRequest {
  expires_in: number;
  token_jti: string;
}

export interface RevokeResponse {
  success: boolean;
}

export interface UpdateRequest {
  access_token_ttl_sec?: number;
  grant_types?: string[];
  name?: string;
  permissions?: string[];
  refresh_token_ttl_sec?: number;
  status?: string;
}

export interface UserJwtRequest {
  claims?: any;
  ttl_secs?: number;
  user_code: string;
  user_id: string;
}

export interface UserJwtResponse {
  access_token: string;
  expires_in: number;
}

export interface ApiResponse<T = any> {
  code: string;
  message: string;
  data?: T;
  request_id?: string;
}
