// 错误归一：audit-service 走 common-error 的 {code,message,request_id} JSON，
// 网关层错误是纯文本。两种形态都归一为 AuditServicesError。
/** 解析错误响应体：优先识别 {code,message,request_id} JSON，否则视作纯文本。 */
export function parseErrorBody(status, rawBody) {
    const text = rawBody.trim();
    if (text === "") {
        return { message: `HTTP ${status}` };
    }
    try {
        const parsed = JSON.parse(text);
        if (parsed !== null && typeof parsed === "object" && typeof parsed.message === "string") {
            const obj = parsed;
            return {
                code: typeof obj.code === "string" ? obj.code : undefined,
                message: obj.message,
                requestId: typeof obj.request_id === "string" ? obj.request_id : undefined,
                body: parsed,
            };
        }
    }
    catch {
        // 非 JSON，按纯文本处理
    }
    return { message: text };
}
/** audit-services SDK 的统一错误类型。 */
export class AuditServicesError extends Error {
    status;
    /** 错误码（如 "AUD_LOG_1001"），仅当服务端返回结构化错误体时存在。 */
    code;
    requestId;
    body;
    constructor(status, message, options = {}) {
        super(message);
        this.name = "AuditServicesError";
        this.status = status;
        this.code = options.code;
        this.requestId = options.requestId;
        this.body = options.body;
    }
    static from(status, rawBody) {
        const { code, message, requestId, body } = parseErrorBody(status, rawBody);
        return new AuditServicesError(status, message, { code, requestId, body });
    }
}
//# sourceMappingURL=errors.js.map