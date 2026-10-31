import { HttpCore } from "./http.js";
import type { DownloadResult, FileQuery, FileInfo, PagedFiles, UploadFileMeta } from "./types.js";
export declare class FileFilesApi {
    private readonly http;
    constructor(http: HttpCore);
    /**
     * multipart 上传：POST /v1/file/upload。
     * 服务端上限默认 100 MiB（超出返回 413 "file too large"）。
     */
    upload(meta: UploadFileMeta, file: Blob | Uint8Array | ArrayBuffer, filename?: string): Promise<FileInfo>;
    /** 分页查询文件：GET /v1/file/list。 */
    page(query?: FileQuery): Promise<PagedFiles>;
    /** 文件元数据：GET /v1/file/{id}/info。 */
    info(id: string): Promise<FileInfo>;
    /** 下载字节：GET /v1/file/{id}，解析 Content-Disposition 文件名。 */
    download(id: string): Promise<DownloadResult>;
    /** 删除文件：DELETE /v1/file/{id} → {success:true}。 */
    remove(id: string): Promise<{
        success: boolean;
    }>;
}
//# sourceMappingURL=files.d.ts.map