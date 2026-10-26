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

  /** 把 axios 错误统一收口成 HttpError，保证调用方拿到的 code / message 一致 */
  function buildError(
    error: AxiosError<ErrorBody>,
    body: ErrorBody,
    status?: number
  ): HttpError {
    const code = body.code;
    const rawMessage = body.message ?? body.msg ?? error.message ?? "请求失败";
    const message = code ? (translateError?.(code) ?? rawMessage) : rawMessage;
    return new HttpError(message, {
      code,
      status,
      data: body.data ?? error.response?.data
    });
  }

  function isWhiteListed(url: string): boolean {
    // 去掉 query string 再匹配，避免带参时白名单失效
    const path = url.split("?")[0];
    return authWhiteList.some(item => path.endsWith(item));
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
      const original = error.config as
        | (InternalAxiosRequestConfig & { skipAuth?: boolean; _prismRetried?: boolean })
        | undefined;
      const status = error.response?.status;
      const body = readErrorBody(error.response?.data);

      // 只有 401（未认证）才刷新 token；403 是权限不足，刷新没有意义。
      // 白名单与 skipAuth 的请求（登录、刷新）绝不参与刷新链路，否则会自锁。
      const canRefresh =
        status === 401 &&
        !!original &&
        !original.skipAuth &&
        !original._prismRetried &&
        !isWhiteListed(original.url ?? "") &&
        !!refreshToken;

      if (canRefresh) {
        // 一个请求最多刷新重放一次，避免持续 401 时无限刷新直到 OOM
        original._prismRetried = true;

        if (!refreshing) {
          refreshing = true;
          try {
            const token = await runRefresh();
            if (token) {
              original.headers.set("Authorization", `Bearer ${token}`);
              flushPending(token);
              return client.request(original);
            }
            // 抛出的 HttpError 会被下面的 catch 统一收口（onUnauthorized 只在 catch 里调一次）
            throw buildError(error, body, status);
          } catch (refreshError) {
            const failure =
              refreshError instanceof HttpError
                ? refreshError
                : buildError(error, body, status);
            flushPending(undefined, failure);
            onUnauthorized?.();
            throw failure;
          } finally {
            refreshing = false;
          }
        }

        // 已有刷新在进行：挂起，等刷新结果
        const token = await new Promise<string | undefined | null>((resolve, reject) => {
          pending.push({ resolve, reject });
        });
        original.headers.set("Authorization", `Bearer ${token}`);
        return client.request(original);
      }

      throw buildError(error, body, status);
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
