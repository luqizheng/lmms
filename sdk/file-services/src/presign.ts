// 预签名 URL（file-service，经网关 /v1/file/presign，需要鉴权）。

import type { HttpCore } from "./http.js";
import type { CreatePresignUrl, PresignUrlResponse } from "./types.js";

export class FilePresignApi {
  constructor(private readonly http: HttpCore) {}

  /** 创建预签名 URL：POST /v1/file/presign。过期返回 401 "presign expired"。 */
  create(request: CreatePresignUrl): Promise<PresignUrlResponse> {
    return this.http.post<PresignUrlResponse>("/v1/file/presign", request, { auth: true });
  }
}
