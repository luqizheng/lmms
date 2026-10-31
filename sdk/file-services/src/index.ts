// file-services：lmms file-service SDK（multipart 上传 / 二进制下载 / 文件管理 / 预签名）。
// 零运行时依赖，原生 fetch + FormData/Blob，浏览器 / Node ≥18 通用；面向 api-gateway 的 /v1/file 公开路径。

import { FileFilesApi } from "./files.js";
import { FilePresignApi } from "./presign.js";
import { HttpCore, type FileServicesOptions } from "./http.js";

export * from "./types.js";
export * from "./errors.js";
export { DEFAULT_BASE_URL, HttpCore, parseDispositionFilename, type FileServicesOptions, type RequestOptions, type BytesResponse } from "./http.js";
export { FileFilesApi } from "./files.js";
export { FilePresignApi } from "./presign.js";

export interface FileServices {
  /** 上传 / 列表 / 元数据 / 下载 / 删除。 */
  readonly files: FileFilesApi;
  /** 预签名 URL。 */
  readonly presign: FilePresignApi;
}

/** 创建 file-service 客户端。 */
export function createFileServices(options: FileServicesOptions = {}): FileServices {
  const http = new HttpCore(options);
  return {
    files: new FileFilesApi(http),
    presign: new FilePresignApi(http),
  };
}
