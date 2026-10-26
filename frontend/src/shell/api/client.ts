import { createHttp, type HttpClient } from "@digua/http";
import { ErrorMessages, ErrorMessagesZhCN } from "@digua/contracts";
import {
  clearSession,
  getAccessToken,
  getRefreshToken,
  saveSession,
  type StoredUser
} from "./token-storage";

export interface LoginResult {
  access_token: string;
  refresh_token: string;
  token_type: string;
  expires_in: number;
  user: StoredUser;
}

/** 业务错误码 → 中文文案（找不到时退回英文，最后退回 code 本身） */
export function translateError(code: string): string {
  const zh = (ErrorMessagesZhCN as Record<string, string>)[code];
  if (zh) return zh;
  const en = (ErrorMessages as Record<string, string>)[code];
  return en ?? code;
}

/** token 彻底失效时的钩子 —— 由 store 在启动时注入（避免与 store 循环依赖） */
let unauthorizedHandler: (() => void) | undefined;

export function onUnauthorized(handler: () => void) {
  unauthorizedHandler = handler;
}

/**
 * 壳的 HTTP 客户端。
 *
 * @digua/http 是框架无关的：token 存取、刷新、文案翻译都在这里注入。
 */
export const http: HttpClient = createHttp({
  baseURL: import.meta.env.VITE_API_BASE ?? "/api",
  getToken: () => getAccessToken(),
  refreshToken: async () => {
    const refreshToken = getRefreshToken();
    if (!refreshToken) return undefined;
    try {
      const result = await http.post<LoginResult>("/v1/iam/auth/refresh", {
        refresh_token: refreshToken
      });
      saveSession({
        accessToken: result.access_token,
        refreshToken: result.refresh_token,
        expiresIn: result.expires_in,
        user: result.user
      });
      return result.access_token;
    } catch {
      return undefined;
    }
  },
  onUnauthorized: () => {
    clearSession();
    unauthorizedHandler?.();
  },
  translateError,
  authWhiteList: [
    "/v1/iam/auth/login",
    "/v1/iam/auth/refresh",
    "/v1/iam/auth/setup-status"
  ]
});
