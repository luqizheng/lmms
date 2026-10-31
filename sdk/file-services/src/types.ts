// file-service DTO 镜像。字段名以后端 serde 序列化为唯一事实源：
//   - services/file-service/openapi.yaml + src/models/file_info.rs（url 为处理器注入）
// 成功响应为裸 JSON；错误体为纯文本状态码。

export interface FileInfo {
  id: string;
  filename: string;
  size: number;
  /** 服务端存储路径（内部信息，调用方一般用不到） */
  storage_path: string;
  tenant_id: number;
  is_temporary: boolean;
  created_at: string;
  updated_at: string;
  content_type: string | null;
  hash: string | null;
  user_id: string | null;
  expires_at: string | null;
  /** 处理器注入的网关相对下载地址：/v1/file/{id} */
  url: string;
}

export interface FileQuery {
  tenant_id?: number;
  user_id?: string;
  filename?: string;
  page?: number;
  size?: number;
}

export interface PagedFiles {
  items: FileInfo[];
  total: number;
  page: number;
  size: number;
}

/**
 * multipart 上传的 metadata 字段（以 JSON 字符串放进表单）。
 * filename 必填；expires_in_seconds 仅 is_temporary 时有意义。
 */
export interface UploadFileMeta {
  filename: string;
  content_type?: string | null;
  tenant_id?: number | null;
  user_id?: string | null;
  is_temporary?: boolean | null;
  expires_in_seconds?: number | null;
}

/** POST /v1/file/presign 的请求体。operation 实际取值见服务端（如 download）。 */
export interface CreatePresignUrl {
  file_id: string;
  operation: string;
  expires_in_seconds?: number | null;
}

export interface PresignUrlResponse {
  url: string;
  expires_at: string;
  operation: string;
}

/** files.download 的返回：字节 + 解析出的下载信息。 */
export interface DownloadResult {
  bytes: ArrayBuffer;
  contentType: string;
  /** 从 Content-Disposition 解析（filename* 优先），解析不出为 null */
  filename: string | null;
  /** 原始 Content-Disposition 头 */
  contentDisposition: string | null;
}
