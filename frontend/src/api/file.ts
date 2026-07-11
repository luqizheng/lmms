import { http } from "@/utils/http";

// ============== 文件 ==============
// 类型来自 scripts/openapi-codegen.js 生成的 frontend/src/api/file/file.types.ts，
// 运行时调 http.request 走 axios（带 JWT header），不复用 codegen 出的 FileClient。

import type {
  FileInfo,
  PagedFiles,
  CreatePresignUrl,
  PresignUrlResponse,
  UploadFileMeta
} from "./file/file.types";

export type {
  FileInfo,
  PagedFiles,
  CreatePresignUrl,
  PresignUrlResponse,
  UploadFileMeta
};

export interface QueryFilesParams {
  tenant_id?: string;
  user_id?: string;
  filename?: string;
  page?: number;
  size?: number;
}

export function listFiles(params?: QueryFilesParams) {
  return http.request<PagedFiles>("get", "/v1/file/list", { params });
}

export function uploadFile(
  file: File,
  meta: Omit<UploadFileMeta, "filename"> & { filename?: string }
) {
  const fd = new FormData();
  fd.append("metadata", JSON.stringify({ filename: file.name, ...meta }));
  fd.append("file", file);
  return http.request<FileInfo>("post", "/v1/file/upload", {
    data: fd,
    headers: { "Content-Type": "multipart/form-data" }
  });
}

export function downloadFile(id: string) {
  return http.request<unknown>("get", `/v1/file/${encodeURIComponent(id)}`);
}

export function deleteFile(id: string) {
  return http.request<void>("delete", `/v1/file/${encodeURIComponent(id)}`);
}

export function getFileInfo(id: string) {
  return http.request<FileInfo>(
    "get",
    `/v1/file/${encodeURIComponent(id)}/info`
  );
}

export function createPresign(body: CreatePresignUrl) {
  return http.request<PresignUrlResponse>("post", "/v1/file/presign", {
    data: body
  });
}

export { fileUrl } from "./file/file-url";
