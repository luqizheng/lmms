import { describe, expect, it } from "vitest";
import { PrueServicesError } from "./errors.js";
import { createPrueServices } from "./index.js";

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
  return createPrueServices({ fetch: impl, getToken: () => "tok" });
}

describe("PrueRoutesApi", () => {
  it("getAsync 解包信封返回路由数组", async () => {
    const route = { path: "/pm", name: "pm", meta: { title: "PM", auths: ["PM_LIST"] }, children: [] };
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { code: 0, message: "ok", data: [route] } }));
    const sdk = makeSdk(impl);
    const routes = await sdk.routes.getAsync();
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/pure/get-async-routes");
    expect(headersOf(calls[0])["Authorization"]).toBe("Bearer tok");
    expect(routes[0].meta?.auths).toEqual(["PM_LIST"]);
  });

  it("信封 code!==0（HTTP 200）抛 PrueServicesError", async () => {
    const { impl } = fakeFetch(() => ({ status: 200, body: { code: 500, message: "boom", data: null } }));
    const sdk = makeSdk(impl);
    const err = await sdk.routes.getAsync().catch((e) => e);
    expect(err).toBeInstanceOf(PrueServicesError);
    expect(err.status).toBe(500);
    expect(err.message).toBe("boom");
  });
});

describe("PrueMenuConfigsApi", () => {
  it("list 解包 {items,total}", async () => {
    const { impl, calls } = fakeFetch(() => ({
      status: 200,
      body: { code: 0, message: "ok", data: { items: [{ id: "c1", version: 1, name: "v1", description: null, status: "draft", is_active: false, created_by: null, created_at: "", updated_at: "", published_at: null, routes_etag: "e1" }], total: 1 } },
    }));
    const sdk = makeSdk(impl);
    const res = await sdk.configs.list();
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/pure/menu-configs");
    expect(res.total).toBe(1);
    expect(res.items[0].routes_etag).toBe("e1");
  });

  it("create POST routes_json；update 携带 if_match_etag", async () => {
    const { impl, calls } = fakeFetch(() => ({
      status: 200,
      body: { code: 0, message: "ok", data: { id: "c1", version: 1, name: "v1", description: null, status: "draft", is_active: false, created_by: null, created_at: "", updated_at: "", published_at: null, routes_etag: "e2" } },
    }));
    const sdk = makeSdk(impl);
    await sdk.configs.create({ name: "v1", routes_json: [{ path: "/" }] });
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ name: "v1", routes_json: [{ path: "/" }] });
    await sdk.configs.update("c1", { name: "v1.1", if_match_etag: "e1" });
    expect(calls[1].init.method).toBe("PUT");
    expect(JSON.parse(String(calls[1].init.body))).toEqual({ name: "v1.1", if_match_etag: "e1" });
  });

  it("409 ETag 冲突：error.body.data 保留 current_etag", async () => {
    const { impl } = fakeFetch(() => ({
      status: 409,
      body: { code: 409, message: "etag conflict", data: { current_etag: "e-999", updated_at: "2026-10-31T00:00:00Z" } },
    }));
    const sdk = makeSdk(impl);
    const err = await sdk.configs.update("c1", { if_match_etag: "stale" }).catch((e) => e);
    expect(err).toBeInstanceOf(PrueServicesError);
    expect(err.status).toBe(409);
    expect((err.body as { data: { current_etag: string } }).data.current_etag).toBe("e-999");
  });

  it("remove 204 / clone / publish 路径", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { code: 0, message: "ok", data: { id: "c2" } } }));
    const sdk = makeSdk(impl);
    await sdk.configs.remove("c1");
    expect(calls[0].init.method).toBe("DELETE");
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/pure/menu-configs/c1");
    await sdk.configs.clone("c1");
    expect(calls[1].url).toBe("http://127.0.0.1:3000/v1/pure/menu-configs/c1/clone");
    await sdk.configs.publish("c1");
    expect(calls[2].url).toBe("http://127.0.0.1:3000/v1/pure/menu-configs/c1/publish");
  });
});

describe("错误归一（prue 形态）", () => {
  it("HTTP 层 {code,message} JSON 错误（code 为数字镜像状态码）", async () => {
    const { impl } = fakeFetch(() => ({ status: 404, body: { code: 404, message: "config not found", data: null } }));
    const sdk = makeSdk(impl);
    const err = await sdk.configs.get("missing").catch((e) => e);
    expect(err).toBeInstanceOf(PrueServicesError);
    expect(err.status).toBe(404);
    expect(err.message).toBe("config not found");
  });
});
