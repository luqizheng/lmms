import axios, {
  type AxiosError,
  type AxiosInstance,
  type AxiosRequestConfig,
  type InternalAxiosRequestConfig
} from "axios";
import {
  HttpError,
  type HttpClient,
  type HttpClientOptions,
  type HttpMethod,
  type RequestConfig
} from "./types";

const DEFAULT_TIMEOUT = 30_000;

/** 后端错误响应体形状（宽松处理，字段缺失时降级） */
interface ErrorBody {
  code?: string;
  message?: string;
  msg?: string;
  data?: unknown;
}

function readErrorBody(payload: unknown): ErrorBody {
  if (payload && typeof payload === "object") return payload as ErrorBody;
  return {};
}

/**
 * 创建 HTTP 客户端。
 *
 * 职责：
 *   1. 注入 Authorization（白名单路径除外）
 *   2. token 过期时单飞刷新并重放原请求
 *   3. 统一把后端错误转成 {@link HttpError}，文案可由消费方翻译
 */
export function createHttp(options: HttpClientOptions = {}): HttpClient {
  const {
    baseURL = "",
    timeout = DEFAULT_TIMEOUT,
    headers,
    getToken,
    refreshToken,
    onUnauthorized,
    translateError,
    authWhiteList = [],
    instance
  } = options;

  const client: AxiosInstance =
    instance ??
    axios.create({
      baseURL,
      timeout,
      headers: {
        Accept: "application/json, text/plain, */*",
        "Content-Type": "application/json",
        ...headers
      }
    });

  /** 刷新中的挂起请求队列 —— 保证同一时刻只有一次刷新 */
  let refreshing = false;
  const pending: Array<{
    resolve: (token: string | undefined | null) => void;
    reject: (error: unknown) => void;
  }> = [];

  function isWhiteListed(url: string): boolean {
    return authWhiteList.some(item => url.endsWith(item));
  }

  client.interceptors.request.use(config => {
    const custom = config as InternalAxiosRequestConfig & { skipAuth?: boolean };
    if (custom.skipAuth || isWhiteListed(config.url ?? "")) return config;

    // 刷新后重放的请求已经带了新 token，不要用旧值覆盖
    if (config.headers.get("Authorization")) return config;

    const token = getToken?.();
    if (token) {
      config.headers.set("Authorization", `Bearer ${token}`);
    }
    return config;
  });

  async function runRefresh(): Promise<string | undefined | null> {
    if (!refreshToken) return undefined;
    try {
      return await refreshToken();
    } catch {
      return undefined;
    }
  }

  function flushPending(token: string | undefined | null, error?: unknown) {
    while (pending.length) {
      const next = pending.shift()!;
      if (error) next.reject(error);
      else next.resolve(token);
    }
  }

  client.interceptors.response.use(
    response => response,
    async (error: AxiosError<ErrorBody>) => {
      const original = error.config as (InternalAxiosRequestConfig & { skipAuth?: boolean }) | undefined;
      const status = error.response?.status;
      const body = readErrorBody(error.response?.data);

      // token 过期 / 未授权：刷新一次并重放原请求
      const shouldRefresh = (status === 401 || status === 403) && !!original && !original.skipAuth;

      if (shouldRefresh && refreshToken) {
        if (!refreshing) {
          refreshing = true;
          try {
            const token = await runRefresh();
            if (token) {
              original.headers.set("Authorization", `Bearer ${token}`);
              flushPending(token);
              return client.request(original);
            }
            flushPending(undefined, error);
          } catch (refreshError) {
            flushPending(undefined, refreshError);
          } finally {
            refreshing = false;
          }
        } else {
          const token = await new Promise<string | undefined | null>((resolve, reject) => {
            pending.push({ resolve, reject });
          });
          original.headers.set("Authorization", `Bearer ${token}`);
          return client.request(original);
        }

        onUnauthorized?.();
      }

      const code = body.code;
      const rawMessage = body.message ?? body.msg ?? error.message ?? "请求失败";
      const message = code ? (translateError?.(code) ?? rawMessage) : rawMessage;

      throw new HttpError(message, {
        code,
        status,
        data: body.data ?? error.response?.data
      });
    }
  );

  async function request<T>(
    method: HttpMethod,
    url: string,
    params?: unknown,
    config: RequestConfig = {}
  ): Promise<T> {
    const response = await client.request<unknown>({
      ...config,
      method,
      url,
      ...(method === "get" || method === "delete" || method === "head"
        ? { params }
        : { data: params })
    });
    return response.data as T;
  }

  return {
    request,
    get: (url, params, config) => request("get", url, params, config),
    post: (url, params, config) => request("post", url, params, config),
    put: (url, params, config) => request("put", url, params, config),
    patch: (url, params, config) => request("patch", url, params, config),
    delete: (url, params, config) => request("delete", url, params, config)
  };
}
