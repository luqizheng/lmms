/** Local file-service address; production callers should inject the internal service URL. */
export declare const DEFAULT_BASE_URL = "http://127.0.0.1:8082";
export interface FileServicesOptions {
    /** Internal file-service base URL (default http://127.0.0.1:8082). Never point this SDK at the public gateway. */
    baseUrl?: string;
    /** Injectable fetch implementation (tests / server runtimes). */
    fetch?: typeof fetch;
    /** Service JWT with aud=file-service. Never pass an end-user access token. */
    getToken?: () => string | undefined | Promise<string | undefined>;
    /** Additional request headers. */
    headers?: Record<string, string>;
}
export interface RequestOptions {
    params?: object;
    body?: unknown;
    auth?: boolean;
    contentType?: string;
    response?: "json" | "text" | "bytes";
}
export interface BytesResponse {
    bytes: ArrayBuffer;
    contentType: string;
    contentDisposition: string | null;
}
export declare function parseDispositionFilename(disposition: string | null): string | null;
export declare class HttpCore {
    private readonly baseUrl;
    private readonly fetchImpl;
    private readonly getToken?;
    private readonly headers;
    constructor(options: FileServicesOptions);
    request<T>(method: string, path: string, options?: RequestOptions): Promise<T>;
    get<T>(path: string, options?: RequestOptions): Promise<T>;
    post<T>(path: string, body?: unknown, options?: RequestOptions): Promise<T>;
    put<T>(path: string, body?: unknown, options?: RequestOptions): Promise<T>;
    delete<T>(path: string, options?: RequestOptions): Promise<T>;
}
//# sourceMappingURL=http.d.ts.map