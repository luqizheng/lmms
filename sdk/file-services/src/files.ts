// 文件管理（file-service，经网关 /v1/file/**，需要鉴权）。
// 上传为 multipart/form-data：字段 metadata（UploadFileMeta 的 JSON 串）+ 字段 file（字节）。
// 下载返回原始字节（Content-Type 为上游存储类型，回退 application/octet-stream）。

import { HttpCore, parseDispositionFilename, type BytesResponse } from "./http.js";
import type { DownloadResult, FileQuery, FileInfo, PagedFiles, UploadFileMeta } from "./types.js";

export class FileFilesApi {
  constructor(private readonly http: HttpCore) {}

  /**
   * multipart 上传：POST /v1/file/upload。
   * 服务端上限默认 100 MiB（超出返回 413 "file too large"）。
   */
  async upload(meta: UploadFileMeta, file: Blob | Uint8Array | ArrayBuffer, filename?: string): Promise<FileInfo> {
    const form = new FormData();
    form.append("metadata", JSON.stringify(meta));
    const name = filename ?? meta.filename;
    if (file instanceof Blob) {
      form.append("file", file, name);
    } else if (file instanceof Uint8Array) {
      form.append("file", new Blob([file]), name);
    } else {
      form.append("file", new Blob([new Uint8Array(file)]), name);
    }
    return this.http.post<FileInfo>("/v1/file/upload", form, { auth: true });
  }

  /** 分页查询文件：GET /v1/file/list。 */
  page(query: FileQuery = {}): Promise<PagedFiles> {
    return this.http.get<PagedFiles>("/v1/file/list", { params: query, auth: true });
  }

  /** 文件元数据：GET /v1/file/{id}/info。 */
  async info(id: string): Promise<FileInfo> {
    return this.http.get<FileInfo>(`/v1/file/${encodeURIComponent(id)}/info`, { auth: true });
  }

  /** 下载字节：GET /v1/file/{id}，解析 Content-Disposition 文件名。 */
  async download(id: string): Promise<DownloadResult> {
    const res = await this.http.get<BytesResponse>(`/v1/file/${encodeURIComponent(id)}`, {
      auth: true,
      response: "bytes",
    });
    return {
      bytes: res.bytes,
      contentType: res.contentType,
      filename: parseDispositionFilename(res.contentDisposition),
      contentDisposition: res.contentDisposition,
    };
  }

  /** 删除文件：DELETE /v1/file/{id} → {success:true}。 */
  remove(id: string): Promise<{ success: boolean }> {
    return this.http.delete<{ success: boolean }>(`/v1/file/${encodeURIComponent(id)}`, { auth: true });
  }
}
