import { describe, expect, it } from "vitest";
import { ConfigServicesError } from "./errors.js";
import { createConfigServices } from "./index.js";

interface RecordedCall {
  url: string;
  init: RequestInit;
}

function fakeFetch(responder: (call: RecordedCall) => { status: number; body: unknown; contentType?: string }) {
  const calls: RecordedCall[] = [];
  const impl = (async (input: string | URL, init?: RequestInit) => {
    const call: RecordedCall = { url: String(input), init: init ?? {} };
    calls.push(call);
    const res = responder(call);
    const text =
      typeof res.body === "string" ? res.body : res.body === undefined || res.body === null ? "" : JSON.stringify(res.body);
    return new Response(res.status === 204 ? null : text, {
      status: res.status,
      headers: { "content-type": res.contentType ?? "application/json" },
    });
  }) as typeof fetch;
  return { impl, calls };
}

function headersOf(call: RecordedCall): Record<string, string> {
  return call.init.headers as Record<string, string>;
}

function makeSdk(impl: typeof fetch) {
  return createConfigServices({ fetch: impl, getToken: () => "tok" });
}

describe("ConfigNamespacesApi", () => {
  it("page 按 app/env 过滤并携带 Bearer", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { items: [], total: 0, page: 1, size: 20 } }));
    const sdk = makeSdk(impl);
    await sdk.namespaces.page({ app: "ai-pm", env: "prod", page: 1, size: 20 });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/config/namespace?app=ai-pm&env=prod&page=1&size=20");
    expect(headersOf(calls[0])["Authorization"]).toBe("Bearer tok");
  });

  it("create POST 请求体", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { id: "ns1", app: "a", env: "dev" } }));
    const sdk = makeSdk(impl);
    await sdk.namespaces.create({ app: "ai-pm", env: "dev" });
    expect(calls[0].init.method).toBe("POST");
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ app: "ai-pm", env: "dev" });
  });

  it("remove 返回 {success:true}", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { success: true } }));
    const sdk = makeSdk(impl);
    const res = await sdk.namespaces.remove("ns1");
    expect(calls[0].init.method).toBe("DELETE");
    expect(res.success).toBe(true);
  });
});

describe("ConfigItemsApi", () => {
  it("page 按 namespace_id/key 过滤", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { items: [], total: 0, page: 1, size: 20 } }));
    const sdk = makeSdk(impl);
    await sdk.items.page({ namespace_id: "ns1", key: "db.url", page: 1, size: 20 });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/config/item?namespace_id=ns1&key=db.url&page=1&size=20");
  });

  it("秘密项掩码字段透传（value_masked）", async () => {
    const { impl } = fakeFetch(() => ({
      status: 200,
      body: { id: "i1", namespace_id: "ns1", key: "db.pwd", value: "********", value_type: "string", is_secret: true, created_at: "", updated_at: "", description: null, value_masked: true },
    }));
    const sdk = makeSdk(impl);
    const item = await sdk.items.get("i1");
    expect(item.value_masked).toBe(true);
    expect(item.value).toBe("********");
  });
});

describe("ConfigTransferApi", () => {
  it("exportConfig 以文本模式返回映射", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: '{"db.url":"postgres://x"}', contentType: "text/plain" }));
    const sdk = makeSdk(impl);
    const text = await sdk.transfer.exportConfig("ns1");
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/config/export/ns1");
    expect(JSON.parse(text)["db.url"]).toBe("postgres://x");
  });

  it("importConfig 以 text/plain 发送 JSON 映射", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { success: true, count: 2 } }));
    const sdk = makeSdk(impl);
    const res = await sdk.transfer.importConfig("ns1", { k1: "v1", k2: "v2" });
    expect(headersOf(calls[0])["Content-Type"]).toBe("text/plain");
    expect(calls[0].init.body).toBe(JSON.stringify({ k1: "v1", k2: "v2" }));
    expect(res.count).toBe(2);
  });

  it("watch 长轮询参数", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { namespace_id: "ns1", changed: false, data: [] } }));
    const sdk = makeSdk(impl);
    const res = await sdk.transfer.watch("ns1", 30);
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/config/watch?namespace_id=ns1&timeout=30");
    expect(res.changed).toBe(false);
  });
});

describe("错误归一", () => {
  it("纯文本错误体（config 路由处理器实际行为）", async () => {
    const { impl } = fakeFetch(() => ({ status: 404, body: "config item not found", contentType: "text/plain" }));
    const sdk = makeSdk(impl);
    const err = await sdk.items.get("missing").catch((e) => e);
    expect(err).toBeInstanceOf(ConfigServicesError);
    expect(err.status).toBe(404);
    expect(err.message).toBe("config item not found");
    expect(err.code).toBeUndefined();
  });
});
