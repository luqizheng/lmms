// 错误归一：notification-service 的错误体是 { "error": string }，
// 上游错误再包一层 { error: "upstream", code, message }（第三种服务约定）。
// 为稳妥起见，{code,message} 形态与纯文本也一并归一。

export interface NormalizedError {
  code?: string;
  message: string;
  body?: unknown;
}

/** 解析错误响应体：{error}（含 upstream 包装）、{code,message}、纯文本。 */
export function parseErrorBody(status: number, rawBody: string): NormalizedError {
  const text = rawBody.trim();
  if (text === "") {
    return { message: `HTTP ${status}` };
  }
  try {
    const parsed: unknown = JSON.parse(text);
    if (parsed !== null && typeof parsed === "object") {
      const obj = parsed as { error?: unknown; code?: unknown; message?: unknown };
      // notification 形态：{error: "..."} 或 {error: "upstream", code, message}
      if (typeof obj.error === "string") {
        const code = typeof obj.code === "string" ? obj.code : undefined;
        const message = typeof obj.message === "string" && obj.error === "upstream" ? obj.message : obj.error;
        return { code, message, body: parsed };
      }
      // 通用 {code,message} 形态
      if (typeof obj.message === "string") {
        return {
          code: typeof obj.code === "string" ? obj.code : undefined,
          message: obj.message,
          body: parsed,
        };
      }
    }
  } catch {
    // 非 JSON，按纯文本处理
  }
  return { message: text };
}

/** notification-services SDK 的统一错误类型。 */
export class NotificationServicesError extends Error {
  readonly status: number;
  readonly code?: string;
  readonly body?: unknown;

  constructor(status: number, message: string, options: { code?: string; body?: unknown } = {}) {
    super(message);
    this.name = "NotificationServicesError";
    this.status = status;
    this.code = options.code;
    this.body = options.body;
  }

  static from(status: number, rawBody: string): NotificationServicesError {
    const { code, message, body } = parseErrorBody(status, rawBody);
    return new NotificationServicesError(status, message, { code, body });
  }
}
