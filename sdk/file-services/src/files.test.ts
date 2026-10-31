import { describe, expect, it } from "vitest";
import { createFileServices, parseDispositionFilename } from "./index.js";

interface RecordedCall {
  url: string;
  init: RequestInit;
}

function fakeFetch(responder: (call: RecordedCall) => { status: number; body?: unknown; contentType?: string; headers?: Record<string, string> }) {
  const calls: RecordedCall[] = [];
  const impl = (async (input: string | URL, init?: RequestInit) => {
    const call: RecordedCall = { url: String(input), init: init ?? {} };
    calls.push(call);
    const res = responder(call);
    if (res.body instanceof ArrayBuffer || res.body === undefined) {
      return new Response((res.body as ArrayBuffer | undefined) ?? null, {
        status: res.status,
        headers: {
          "content-type": res.contentType ?? "application/octet-stream",
          ...(res.headers ?? {}),
        },
      });
    }
    const text = typeof res.body === "string" ? res.body : JSON.stringify(res.body);
    return new Response(text, {
      status: res.status,
      headers: { "content-type": res.contentType ?? "application/json", ...(res.headers ?? {}) },
    });
  }) as typeof fetch;
  return { impl, calls };
}

function headersOf(call: RecordedCall): Record<string, string> {
  return call.init.headers as Record<string, string>;
}

function makeSdk(impl: typeof fetch) {
  return createFileServices({ fetch: impl, getToken: () => "tok" });
}

describe("FileFilesApi.upload", () => {
  it("multipart：metadata JSON 串 + file 字段，不设 JSON Content-Type", async () => {
    const { impl, calls } = fakeFetch(() => ({
      status: 200,
      body: { id: "f1", filename: "a.txt", size: 5, storage_path: "/x", tenant_id: 1, is_temporary: false, created_at: "", updated_at: "", url: "/v1/file/f1" },
    }));
    const sdk = makeSdk(impl);
    const info = await sdk.files.upload({ filename: "a.txt", tenant_id: 1 }, new Blob(["hello"], { type: "text/plain" }));
    expect(calls[0].init.method).toBe("POST");
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/file/upload");
    expect(headersOf(calls[0])["Authorization"]).toBe("Bearer tok");
    // fetch 对 FormData 自动设 multipart 边界，SDK 不得覆盖为 JSON
    expect(headersOf(calls[0])["Content-Type"]).toBeUndefined();
    const form = calls[0].init.body as FormData;
    expect(JSON.parse(String(form.get("metadata")))).toEqual({ filename: "a.txt", tenant_id: 1 });
    expect(form.get("file")).toBeInstanceOf(Blob);
    expect(info.url).toBe("/v1/file/f1");
  });

  it("Uint8Array 输入同样走 multipart", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { id: "f2" } }));
    const sdk = makeSdk(impl);
    await sdk.files.upload({ filename: "b.bin" }, new Uint8Array([1, 2, 3]));
    const form = calls[0].init.body as FormData;
    expect(form.get("file")).toBeInstanceOf(Blob);
  });

  it("413 超限错误归一为纯文本 message", async () => {
    const { impl } = fakeFetch(() => ({ status: 413, body: "file too large", contentType: "text/plain" }));
    const sdk = makeSdk(impl);
    await expect(sdk.files.upload({ filename: "big.bin" }, new Uint8Array(8))).rejects.toMatchObject({
      status: 413,
      message: "file too large",
    });
  });
});

describe("FileFilesApi.download", () => {
  it("返回字节 + 解析 filename* 与 Content-Type", async () => {
    const bytes = new TextEncoder().encode("hello-bytes").buffer;
    const { impl, calls } = fakeFetch(() => ({
      status: 200,
      body: bytes as ArrayBuffer,
      contentType: "application/pdf",
      headers: { "content-disposition": `attachment; filename="fallback.txt"; filename*=UTF-8''%E6%8A%A5%E5%91%8A.pdf` },
    }));
    const sdk = makeSdk(impl);
    const res = await sdk.files.download("f1");
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/file/f1");
    expect(new TextDecoder().decode(res.bytes)).toBe("hello-bytes");
    expect(res.contentType).toBe("application/pdf");
    expect(res.filename).toBe("报告.pdf");
  });

  it("无 Content-Disposition 时 filename 为 null", async () => {
    const { impl } = fakeFetch(() => ({ status: 200, body: new TextEncoder().encode("x").buffer as ArrayBuffer }));
    const sdk = makeSdk(impl);
    const res = await sdk.files.download("f2");
    expect(res.filename).toBeNull();
  });
});

describe("FilePresignApi 与其余端点", () => {
  it("presign.create 请求体与响应", async () => {
    const { impl, calls } = fakeFetch(() => ({
      status: 200,
      body: { url: "http://x/signed", expires_at: "2026-11-01T00:00:00Z", operation: "download" },
    }));
    const sdk = makeSdk(impl);
    const res = await sdk.presign.create({ file_id: "f1", operation: "download", expires_in_seconds: 300 });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/file/presign");
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ file_id: "f1", operation: "download", expires_in_seconds: 300 });
    expect(res.operation).toBe("download");
  });

  it("page 组装过滤参数；info/info 路径编码", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { items: [], total: 0, page: 1, size: 20 } }));
    const sdk = makeSdk(impl);
    await sdk.files.page({ user_id: "u1", filename: "报 告.pdf", page: 1, size: 20 });
    expect(calls[0].url).toContain("/v1/file/list?user_id=u1&filename=");
    await sdk.files.info("f/1");
    expect(calls[1].url).toBe("http://127.0.0.1:3000/v1/file/f%2F1/info");
  });

  it("remove DELETE", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { success: true } }));
    const sdk = makeSdk(impl);
    expect((await sdk.files.remove("f1")).success).toBe(true);
    expect(calls[0].init.method).toBe("DELETE");
  });
});

describe("parseDispositionFilename", () => {
  it("filename* 优先于 filename", () => {
    expect(parseDispositionFilename(`attachment; filename="a.txt"; filename*=UTF-8''%E6%8A%A5.pdf`)).toBe("报.pdf");
  });
  it("无头返回 null", () => {
    expect(parseDispositionFilename(null)).toBeNull();
  });
});
