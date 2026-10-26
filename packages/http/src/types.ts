/**
 * @digua/http —— 框架无关的 HTTP 客户端。
 *
 * 设计要点：**不绑定任何 UI 框架**。
 * token 存取、刷新、错误文案翻译都由消费方通过 options 注入，
 * 因此 Vue 壳与 project-manager 可以共用同一份实现。
 */
import type { AxiosInstance, AxiosRequestConfig } from "axios";

export type HttpMethod =
  | "get"
  | "post"
  | "put"
  | "delete"
  | "patch"
  | "head"
  | "options";

export interface RequestConfig extends AxiosRequestConfig {
  /** 跳过鉴权注入（用于登录、刷新等公开端点） */
  skipAuth?: boolean;
}

/** 业务 / 传输错误。code 为后端契约里的错误码字符串。 */
export class HttpError extends Error {
  readonly code?: string;
  readonly status?: number;
  readonly data?: unknown;

  constructor(message: string, options: { code?: string; status?: number; data?: unknown } = {}) {
    super(message);
    this.name = "HttpError";
    this.code = options.code;
    this.status = options.status;
    this.data = options.data;
  }
}

export interface HttpClientOptions {
  /** 请求基址 */
  baseURL?: string;
  /** 超时（毫秒） */
  timeout?: number;
  /** 附加的公共请求头 */
  headers?: Record<string, string>;
  /** 取当前 access token（不加前缀） */
  getToken?: () => string | undefined | null;
  /** 触发 token 刷新，返回新的 access token；无法刷新时返回 undefined */
  refreshToken?: () => Promise<string | undefined | null>;
  /** token 彻底失效（刷新失败）时的回调 */
  onUnauthorized?: () => void;
  /** 业务错误码 → 用户文案；返回 undefined 表示用后端原文 */
  translateError?: (code: string) => string | undefined;
  /** 不携带 Authorization 的路径后缀白名单 */
  authWhiteList?: string[];
  /** 自定义 axios 实例（便于测试与统一拦截器） */
  instance?: AxiosInstance;
}

export interface HttpClient {
  request<T = unknown>(
    method: HttpMethod,
    url: string,
    params?: unknown,
    config?: RequestConfig
  ): Promise<T>;
  get<T = unknown>(url: string, params?: unknown, config?: RequestConfig): Promise<T>;
  post<T = unknown>(url: string, params?: unknown, config?: RequestConfig): Promise<T>;
  put<T = unknown>(url: string, params?: unknown, config?: RequestConfig): Promise<T>;
  patch<T = unknown>(url: string, params?: unknown, config?: RequestConfig): Promise<T>;
  delete<T = unknown>(url: string, params?: unknown, config?: RequestConfig): Promise<T>;
}
