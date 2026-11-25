import { HttpCore } from "./http.js";
import type { DownloadResult, FileQuery, FileInfo, PagedFiles, UploadFileMeta } from "./types.js";
export declare class FileFilesApi {
    private readonly http;
    constructor(http: HttpCore);
    /** Multipart upload: POST /file/upload. Default service limit is 100 MiB. */
    upload(meta: UploadFileMeta, file: Blob | Uint8Array | ArrayBuffer, filename?: string): Promise<FileInfo>;
    page(query?: FileQuery): Promise<PagedFiles>;
    info(id: string): Promise<FileInfo>;
    download(id: string): Promise<DownloadResult>;
    remove(id: string): Promise<{
        success: boolean;
    }>;
}
//# sourceMappingURL=files.d.ts.map