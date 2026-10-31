export interface NormalizedError {
    code?: string;
    message: string;
    requestId?: string;
    body?: unknown;
}
/** 解析错误响应体：优先识别 {code,message,request_id} JSON，否则视作纯文本。 */
export declare function parseErrorBody(status: number, rawBody: string): NormalizedError;
/** audit-services SDK 的统一错误类型。 */
export declare class AuditServicesError extends Error {
    readonly status: number;
    /** 错误码（如 "AUD_LOG_1001"），仅当服务端返回结构化错误体时存在。 */
    readonly code?: string;
    readonly requestId?: string;
    readonly body?: unknown;
    constructor(status: number, message: string, options?: {
        code?: string;
        requestId?: string;
        body?: unknown;
    });
    static from(status: number, rawBody: string): AuditServicesError;
}
//# sourceMappingURL=errors.d.ts.map