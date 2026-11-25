// Server-to-server request core for file-service.
// This package must not be used from browsers: file-service accepts only service-jwt
// tokens with aud=file-service. End-user authorization belongs in the calling business service.
import { FileServicesError } from "./errors.js";
/** Local file-service address; production callers should inject the internal service URL. */
export const DEFAULT_BASE_URL = "http://127.0.0.1:8082";
export function parseDispositionFilename(disposition) {
    if (!disposition)
        return null;
    const utf8 = /filename\*=(?:UTF-8|utf-8)''([^;]+)/.exec(disposition);
    if (utf8) {
        try {
            return decodeURIComponent(utf8[1].trim().replace(/^"|"$/g, ""));
        }
        catch { /* fall back to filename */ }
    }
    const plain = /filename="?([^";]+)"?/.exec(disposition);
    return plain ? plain[1].trim() : null;
}
function buildUrl(baseUrl, path, params) {
    let url = baseUrl + path;
    if (params) {
        const search = new URLSearchParams();
        for (const [key, value] of Object.entries(params)) {
            if (value === undefined || value === null || value === "")
                continue;
            search.set(key, String(value));
        }
        const qs = search.toString();
        if (qs)
            url += (url.includes("?") ? "&" : "?") + qs;
    }
    return url;
}
export class HttpCore {
    baseUrl;
    fetchImpl;
    getToken;
    headers;
    constructor(options) {
        this.baseUrl = (options.baseUrl ?? DEFAULT_BASE_URL).replace(/\/+$/, "");
        this.fetchImpl = options.fetch ?? fetch.bind(globalThis);
        this.getToken = options.getToken;
        this.headers = { ...options.headers };
    }
    async request(method, path, options = {}) {
        const { params, body, auth = false, contentType, response = "json" } = options;
        const url = buildUrl(this.baseUrl, path, params);
        const headers = { ...this.headers };
        const isFormData = typeof FormData !== "undefined" && body instanceof FormData;
        if (body !== undefined && !isFormData) {
            headers["Content-Type"] = contentType ?? (typeof body === "string" ? "text/plain" : "application/json");
        }
        if (auth) {
            const token = await this.getToken?.();
            if (token)
                headers["Authorization"] = `Bearer ${token}`;
        }
        const res = await this.fetchImpl(url, {
            method,
            headers,
            body: body === undefined ? undefined : isFormData ? body : typeof body === "string" ? body : JSON.stringify(body),
        });
        if (!res.ok)
            throw FileServicesError.from(res.status, await res.text());
        if (response === "bytes") {
            return {
                bytes: await res.arrayBuffer(),
                contentType: res.headers.get("content-type") ?? "application/octet-stream",
                contentDisposition: res.headers.get("content-disposition"),
            };
        }
        const text = await res.text();
        if (response === "text")
            return text;
        if (res.status === 204 || text === "")
            return undefined;
        const ct = res.headers.get("content-type") ?? "";
        return ct.includes("application/json") ? JSON.parse(text) : text;
    }
    get(path, options = {}) { return this.request("GET", path, options); }
    post(path, body, options = {}) { return this.request("POST", path, { ...options, body }); }
    put(path, body, options = {}) { return this.request("PUT", path, { ...options, body }); }
    delete(path, options = {}) { return this.request("DELETE", path, options); }
}
//# sourceMappingURL=http.js.map