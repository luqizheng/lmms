// Internal file-service API. Requires service-jwt (aud=file-service); never expose to browsers.
// Upload uses multipart/form-data: metadata (JSON string) + file (bytes).

import { HttpCore, parseDispositionFilename, type BytesResponse } from "./http.js";
import type { DownloadResult, FileQuery, FileInfo, PagedFiles, UploadFileMeta } from "./types.js";

export class FileFilesApi {
  constructor(private readonly http: HttpCore) {}

  /** Multipart upload: POST /file/upload. Default service limit is 100 MiB. */
  async upload(meta: UploadFileMeta, file: Blob | Uint8Array | ArrayBuffer, filename?: string): Promise<FileInfo> {
    const form = new FormData();
    form.append("metadata", JSON.stringify(meta));
    const name = filename ?? meta.filename;
    if (file instanceof Blob) form.append("file", file, name);
    else if (file instanceof Uint8Array) form.append("file", new Blob([file]), name);
    else form.append("file", new Blob([new Uint8Array(file)]), name);
    return this.http.post<FileInfo>("/file/upload", form, { auth: true });
  }

  page(query: FileQuery = {}): Promise<PagedFiles> {
    return this.http.get<PagedFiles>("/file/list", { params: query, auth: true });
  }

  info(id: string): Promise<FileInfo> {
    return this.http.get<FileInfo>(`/file/${encodeURIComponent(id)}/info`, { auth: true });
  }

  async download(id: string): Promise<DownloadResult> {
    const res = await this.http.get<BytesResponse>(`/file/${encodeURIComponent(id)}`, { auth: true, response: "bytes" });
    return {
      bytes: res.bytes,
      contentType: res.contentType,
      filename: parseDispositionFilename(res.contentDisposition),
      contentDisposition: res.contentDisposition,
    };
  }

  remove(id: string): Promise<{ success: boolean }> {
    return this.http.delete<{ success: boolean }>(`/file/${encodeURIComponent(id)}`, { auth: true });
  }
}
