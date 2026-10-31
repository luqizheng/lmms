/** api-gateway 默认基地址（本机联调）。 */
export declare const DEFAULT_BASE_URL = "http://127.0.0.1:3000";
export interface NotificationServicesOptions {
    /** api-gateway 基地址，默认 http://127.0.0.1:3000。支持相对路径（浏览器同域反代场景）。 */
    baseUrl?: string;
    /** 可注入 fetch 实现（测试 / SSR / 自定义代理）。默认全局 fetch。 */
    fetch?: typeof fetch;
    /** 受保护端点附加的 Bearer token；返回 undefined 时该请求不带 Authorization 头。 */
    getToken?: () => string | undefined | Promise<string | undefined>;
    /** 附加在所有请求上的自定义头。 */
    headers?: Record<string, string>;
}
export interface RequestOptions {
    /** 查询参数对象（跳过 undefined/null/空串值）。 */
    params?: object;
    /** 对象 → JSON；string → 原样发送（Content-Type 见 contentType）；FormData → multipart 透传。 */
    body?: unknown;
    /** 是否附加 Authorization: Bearer（默认 false）。 */
    auth?: boolean;
    /** 覆盖请求体 Content-Type（string 体默认 text/plain；对象体默认 application/json；FormData 忽略本项）。 */
    contentType?: string;
    /** 响应解析模式：json（默认）/ text / bytes。 */
    response?: "json" | "text" | "bytes";
}
/** response:"bytes" 时的返回结构。 */
export interface BytesResponse {
    bytes: ArrayBuffer;
    contentType: string;
    contentDisposition: string | null;
}
/** 从 Content-Disposition 解析文件名（RFC 5987 filename* 优先，回退 filename，均无则 null）。 */
export declare function parseDispositionFilename(disposition: string | null): string | null;
export declare class HttpCore {
    private readonly baseUrl;
    private readonly fetchImpl;
    private readonly getToken?;
    private readonly headers;
    constructor(options: NotificationServicesOptions);
    request<T>(method: string, path: string, options?: RequestOptions): Promise<T>;
    get<T>(path: string, options?: RequestOptions): Promise<T>;
    post<T>(path: string, body?: unknown, options?: RequestOptions): Promise<T>;
    put<T>(path: string, body?: unknown, options?: RequestOptions): Promise<T>;
    delete<T>(path: string, options?: RequestOptions): Promise<T>;
}
//# sourceMappingURL=http.d.ts.map