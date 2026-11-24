// Server-to-server request core for file-service.
// This package must not be used from browsers: file-service accepts only service-jwt
// tokens with aud=file-service. End-user authorization belongs in the calling business service.

import { FileServicesError } from "./errors.js";

/** Local file-service address; production callers should inject the internal service URL. */
export const DEFAULT_BASE_URL = "http://127.0.0.1:8082";

export interface FileServicesOptions {
  /** Internal file-service base URL (default http://127.0.0.1:8082). Never point this SDK at the public gateway. */
  baseUrl?: string;
  /** Injectable fetch implementation (tests / server runtimes). */
  fetch?: typeof fetch;
  /** Service JWT with aud=file-service. Never pass an end-user access token. */
  getToken?: () => string | undefined | Promise<string | undefined>;
  /** Additional request headers. */
  headers?: Record<string, string>;
}

export interface RequestOptions {
  params?: object;
  body?: unknown;
  auth?: boolean;
  contentType?: string;
  response?: "json" | "text" | "bytes";
}

export interface BytesResponse {
  bytes: ArrayBuffer;
  contentType: string;
  contentDisposition: string | null;
}

export function parseDispositionFilename(disposition: string | null): string | null {
  if (!disposition) return null;
  const utf8 = /filename\*=(?:UTF-8|utf-8)''([^;]+)/.exec(disposition);
  if (utf8) {
    try { return decodeURIComponent(utf8[1].trim().replace(/^"|"$/g, "")); }
    catch { /* fall back to filename */ }
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
  private readonly getToken?: FileServicesOptions["getToken"];
  private readonly headers: Record<string, string>;

  constructor(options: FileServicesOptions) {
    this.baseUrl = (options.baseUrl ?? DEFAULT_BASE_URL).replace(/\/+$/, "");
    this.fetchImpl = options.fetch ?? fetch.bind(globalThis);
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
      body: body === undefined ? undefined : isFormData ? (body as FormData) : typeof body === "string" ? body : JSON.stringify(body),
    });
    if (!res.ok) throw FileServicesError.from(res.status, await res.text());
    if (response === "bytes") {
      return {
        bytes: await res.arrayBuffer(),
        contentType: res.headers.get("content-type") ?? "application/octet-stream",
        contentDisposition: res.headers.get("content-disposition"),
      } as T;
    }
    const text = await res.text();
    if (response === "text") return text as T;
    if (res.status === 204 || text === "") return undefined as T;
    const ct = res.headers.get("content-type") ?? "";
    return ct.includes("application/json") ? JSON.parse(text) as T : text as unknown as T;
  }

  get<T>(path: string, options: RequestOptions = {}): Promise<T> { return this.request<T>("GET", path, options); }
  post<T>(path: string, body?: unknown, options: RequestOptions = {}): Promise<T> { return this.request<T>("POST", path, { ...options, body }); }
  put<T>(path: string, body?: unknown, options: RequestOptions = {}): Promise<T> { return this.request<T>("PUT", path, { ...options, body }); }
  delete<T>(path: string, options: RequestOptions = {}): Promise<T> { return this.request<T>("DELETE", path, options); }
}
