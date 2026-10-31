// prue_backend 是五个 REST 服务中唯一使用 {code,message,data} 信封的（openapi 未建模该包装，
// 以处理器实际输出为准）。SDK 统一解包：code===0 → data；否则抛 PrueServicesError。

import { PrueServicesError } from "./errors.js";

export interface ApiResponse<T> {
  /** 0 = 成功；错误时镜像 HTTP 状态码 */
  code: number;
  message: string;
  data: T;
}

/** 解包 ApiResponse：非 0 抛错（错误体保留在 error.body，409 时含 current_etag 等）。 */
export async function unwrap<T>(call: Promise<ApiResponse<T>>): Promise<T> {
  const res = await call;
  if (res.code !== 0) {
    throw new PrueServicesError(res.code, res.message || `prue error ${res.code}`, { body: res });
  }
  return res.data;
}
