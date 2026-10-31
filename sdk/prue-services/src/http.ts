// 请求核：零依赖 fetch，浏览器 / Node ≥18 通用（iam-services 同款内核 +
// 原始文本体 / 文本与字节响应模式扩展）。
// SDK 不做自动刷新——单飞刷新留在消费方的 HTTP 层（如 @digua/http）；
// 受保护端点通过 getToken() 注入 Bearer，由应用层决定 token 来源。

import { PrueServicesError } from "./errors.js";

/** api-gateway 默认基地址（本机联调）。 */
export const DEFAULT_BASE_URL = "http://127.0.0.1:3000";

export interface PrueServicesOptions {
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
export function parseDispositionFilename(disposition: string | null): string | null {
  if (!disposition) return null;
  const utf8 = /filename\*=(?:UTF-8|utf-8)''([^;]+)/.exec(disposition);
  if (utf8) {
    try {
      return decodeURIComponent(utf8[1].trim().replace(/^"|"$/g, ""));
    } catch {
      // 解码失败回退普通 filename
    }
  }
  const plain = /filename="?([^";]+)"?/.exec(disposition);
  return plain ? plain[1].trim() : null;
}

function buildUrl(baseUrl: string, path: string, params?: object): string {
  let url = baseUrl + path;
  if (params) {
    const search = new URLSearchParams();
    for (const [key, value] of Object.entries(params)) {
      if (value === undefined || value === null || value === "") continue;
      search.set(key, String(value));
    }
    const qs = search.toString();
    if (qs) url += (url.includes("?") ? "&" : "?") + qs;
  }
  return url;
}

export class HttpCore {
  private readonly baseUrl: string;
  private readonly fetchImpl: typeof fetch;
  private readonly getToken?: PrueServicesOptions["getToken"];
  private readonly headers: Record<string, string>;

  constructor(options: PrueServicesOptions) {
    this.baseUrl = (options.baseUrl ?? DEFAULT_BASE_URL).replace(/\/+$/, "");
    this.fetchImpl = options.fetch ?? fetch;
    this.getToken = options.getToken;
    this.headers = { ...options.headers };
  }

  async request<T>(method: string, path: string, options: RequestOptions = {}): Promise<T> {
    const { params, body, auth = false, contentType, response = "json" } = options;
    const url = buildUrl(this.baseUrl, path, params);

    const headers: Record<string, string> = { ...this.headers };
    const isFormData = typeof FormData !== "undefined" && body instanceof FormData;
    if (body !== undefined && !isFormData) {
      headers["Content-Type"] = contentType ?? (typeof body === "string" ? "text/plain" : "application/json");
    }
    if (auth) {
      const token = await this.getToken?.();
      if (token) headers["Authorization"] = `Bearer ${token}`;
    }

    const res = await this.fetchImpl(url, {
      method,
      headers,
      body:
        body === undefined
          ? undefined
          : isFormData
            ? (body as FormData)
            : typeof body === "string"
              ? body
              : JSON.stringify(body),
    });

    if (!res.ok) {
      throw PrueServicesError.from(res.status, await res.text());
    }
    if (response === "bytes") {
      return {
        bytes: await res.arrayBuffer(),
        contentType: res.headers.get("content-type") ?? "application/octet-stream",
        contentDisposition: res.headers.get("content-disposition"),
      } as T;
    }
    const text = await res.text();
    if (response === "text") {
      return text as T;
    }
    if (res.status === 204 || text === "") {
      return undefined as T;
    }
    const ct = res.headers.get("content-type") ?? "";
    if (ct.includes("application/json")) {
      return JSON.parse(text) as T;
    }
    return text as unknown as T;
  }

  get<T>(path: string, options: RequestOptions = {}): Promise<T> {
    return this.request<T>("GET", path, options);
  }

  post<T>(path: string, body?: unknown, options: RequestOptions = {}): Promise<T> {
    return this.request<T>("POST", path, { ...options, body });
  }

  put<T>(path: string, body?: unknown, options: RequestOptions = {}): Promise<T> {
    return this.request<T>("PUT", path, { ...options, body });
  }

  delete<T>(path: string, options: RequestOptions = {}): Promise<T> {
    return this.request<T>("DELETE", path, options);
  }
}
