// Internal presign API. Requires service-jwt (aud=file-service).

import type { HttpCore } from "./http.js";
import type { CreatePresignUrl, PresignUrlResponse } from "./types.js";

export class FilePresignApi {
  constructor(private readonly http: HttpCore) {}

  /** Create a short-lived GET or PUT URL: POST /file/presign. */
  create(request: CreatePresignUrl): Promise<PresignUrlResponse> {
    return this.http.post<PresignUrlResponse>("/file/presign", request, { auth: true });
  }
}
