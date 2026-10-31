// 请求核：零依赖 fetch，浏览器 / Node ≥18 通用。
// SDK 不做自动刷新——单飞刷新留在消费方的 HTTP 层（如 @digua/http）；
// 受保护端点通过 getToken() 注入 Bearer，由应用层决定 token 来源。

import { IamServicesError } from "./errors.js";

/** api-gateway 默认基地址（本机联调）。 */
export const DEFAULT_BASE_URL = "http://127.0.0.1:3000";

export interface IamServicesOptions {
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
  /** 查询参数对象（跳过 undefined/null/空串值）；传各 API 定义的查询接口即可。 */
  params?: object;
  body?: unknown;
  /** 是否附加 Authorization: Bearer（默认 false）。 */
  auth?: boolean;
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
  private readonly getToken?: IamServicesOptions["getToken"];
  private readonly headers: Record<string, string>;

  constructor(options: IamServicesOptions) {
    this.baseUrl = (options.baseUrl ?? DEFAULT_BASE_URL).replace(/\/+$/, "");
    // bind：浏览器里裸 fetch 以方法调用会 Illegal invocation（Node 无此限制，测试未暴露）
    this.fetchImpl = options.fetch ?? fetch.bind(globalThis);
    this.getToken = options.getToken;
    this.headers = { ...options.headers };
  }

  async request<T>(method: string, path: string, options: RequestOptions = {}): Promise<T> {
    const { params, body, auth = false } = options;
    const url = buildUrl(this.baseUrl, path, params);

    const headers: Record<string, string> = { ...this.headers };
    if (body !== undefined) headers["Content-Type"] = "application/json";
    if (auth) {
      const token = await this.getToken?.();
      if (token) headers["Authorization"] = `Bearer ${token}`;
    }

    const response = await this.fetchImpl(url, {
      method,
      headers,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });

    const text = await response.text();
    if (!response.ok) {
      throw IamServicesError.from(response.status, text);
    }
    if (response.status === 204 || text === "") {
      return undefined as T;
    }
    const contentType = response.headers.get("content-type") ?? "";
    if (contentType.includes("application/json")) {
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
