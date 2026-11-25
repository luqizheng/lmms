// Internal file-service API. Requires service-jwt (aud=file-service); never expose to browsers.
// Upload uses multipart/form-data: metadata (JSON string) + file (bytes).
import { parseDispositionFilename } from "./http.js";
export class FileFilesApi {
    http;
    constructor(http) {
        this.http = http;
    }
    /** Multipart upload: POST /file/upload. Default service limit is 100 MiB. */
    async upload(meta, file, filename) {
        const form = new FormData();
        form.append("metadata", JSON.stringify(meta));
        const name = filename ?? meta.filename;
        if (file instanceof Blob)
            form.append("file", file, name);
        else if (file instanceof Uint8Array)
            form.append("file", new Blob([file]), name);
        else
            form.append("file", new Blob([new Uint8Array(file)]), name);
        return this.http.post("/file/upload", form, { auth: true });
    }
    page(query = {}) {
        return this.http.get("/file/list", { params: query, auth: true });
    }
    info(id) {
        return this.http.get(`/file/${encodeURIComponent(id)}/info`, { auth: true });
    }
    async download(id) {
        const res = await this.http.get(`/file/${encodeURIComponent(id)}`, { auth: true, response: "bytes" });
        return {
            bytes: res.bytes,
            contentType: res.contentType,
            filename: parseDispositionFilename(res.contentDisposition),
            contentDisposition: res.contentDisposition,
        };
    }
    remove(id) {
        return this.http.delete(`/file/${encodeURIComponent(id)}`, { auth: true });
    }
}
//# sourceMappingURL=files.js.map