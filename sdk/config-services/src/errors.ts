// 错误归一：audit-service 走 common-error 的 {code,message,request_id} JSON，
// 网关层错误是纯文本。两种形态都归一为 ConfigServicesError。

export interface NormalizedError {
  code?: string;
  message: string;
  requestId?: string;
  body?: unknown;
}

/** 解析错误响应体：优先识别 {code,message,request_id} JSON，否则视作纯文本。 */
export function parseErrorBody(status: number, rawBody: string): NormalizedError {
  const text = rawBody.trim();
  if (text === "") {
    return { message: `HTTP ${status}` };
  }
  try {
    const parsed: unknown = JSON.parse(text);
    if (parsed !== null && typeof parsed === "object" && typeof (parsed as { message?: unknown }).message === "string") {
      const obj = parsed as { code?: unknown; message: string; request_id?: unknown };
      return {
        code: typeof obj.code === "string" ? obj.code : undefined,
        message: obj.message,
        requestId: typeof obj.request_id === "string" ? obj.request_id : undefined,
        body: parsed,
      };
    }
  } catch {
    // 非 JSON，按纯文本处理
  }
  return { message: text };
}

/** audit-services SDK 的统一错误类型。 */
export class ConfigServicesError extends Error {
  readonly status: number;
  /** 错误码（如 "AUD_LOG_1001"），仅当服务端返回结构化错误体时存在。 */
  readonly code?: string;
  readonly requestId?: string;
  readonly body?: unknown;

  constructor(status: number, message: string, options: { code?: string; requestId?: string; body?: unknown } = {}) {
    super(message);
    this.name = "ConfigServicesError";
    this.status = status;
    this.code = options.code;
    this.requestId = options.requestId;
    this.body = options.body;
  }

  static from(status: number, rawBody: string): ConfigServicesError {
    const { code, message, requestId, body } = parseErrorBody(status, rawBody);
    return new ConfigServicesError(status, message, { code, requestId, body });
  }
}
