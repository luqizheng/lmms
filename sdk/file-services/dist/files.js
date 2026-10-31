// 文件管理（file-service，经网关 /v1/file/**，需要鉴权）。
// 上传为 multipart/form-data：字段 metadata（UploadFileMeta 的 JSON 串）+ 字段 file（字节）。
// 下载返回原始字节（Content-Type 为上游存储类型，回退 application/octet-stream）。
import { parseDispositionFilename } from "./http.js";
export class FileFilesApi {
    http;
    constructor(http) {
        this.http = http;
    }
    /**
     * multipart 上传：POST /v1/file/upload。
     * 服务端上限默认 100 MiB（超出返回 413 "file too large"）。
     */
    async upload(meta, file, filename) {
        const form = new FormData();
        form.append("metadata", JSON.stringify(meta));
        const name = filename ?? meta.filename;
        if (file instanceof Blob) {
            form.append("file", file, name);
        }
        else if (file instanceof Uint8Array) {
            form.append("file", new Blob([file]), name);
        }
        else {
            form.append("file", new Blob([new Uint8Array(file)]), name);
        }
        return this.http.post("/v1/file/upload", form, { auth: true });
    }
    /** 分页查询文件：GET /v1/file/list。 */
    page(query = {}) {
        return this.http.get("/v1/file/list", { params: query, auth: true });
    }
    /** 文件元数据：GET /v1/file/{id}/info。 */
    async info(id) {
        return this.http.get(`/v1/file/${encodeURIComponent(id)}/info`, { auth: true });
    }
    /** 下载字节：GET /v1/file/{id}，解析 Content-Disposition 文件名。 */
    async download(id) {
        const res = await this.http.get(`/v1/file/${encodeURIComponent(id)}`, {
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
    remove(id) {
        return this.http.delete(`/v1/file/${encodeURIComponent(id)}`, { auth: true });
    }
}
//# sourceMappingURL=files.js.map