export interface NormalizedError {
    code?: string;
    message: string;
    body?: unknown;
}
/** 解析错误响应体：{error}（含 upstream 包装）、{code,message}、纯文本。 */
export declare function parseErrorBody(status: number, rawBody: string): NormalizedError;
/** notification-services SDK 的统一错误类型。 */
export declare class NotificationServicesError extends Error {
    readonly status: number;
    readonly code?: string;
    readonly body?: unknown;
    constructor(status: number, message: string, options?: {
        code?: string;
        body?: unknown;
    });
    static from(status: number, rawBody: string): NotificationServicesError;
}
//# sourceMappingURL=errors.d.ts.map