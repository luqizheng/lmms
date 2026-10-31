import { FileFilesApi } from "./files.js";
import { FilePresignApi } from "./presign.js";
import { type FileServicesOptions } from "./http.js";
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
export declare function createFileServices(options?: FileServicesOptions): FileServices;
//# sourceMappingURL=index.d.ts.map