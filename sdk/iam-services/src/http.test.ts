import { describe, expect, it } from "vitest";
import { IamServicesError } from "./errors.js";
import { HttpCore } from "./http.js";

interface RecordedCall {
  url: string;
  init: RequestInit;
}

type Responder = (call: RecordedCall) => { status: number; body: unknown; contentType?: string };

function fakeFetch(responder: Responder) {
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

describe("HttpCore", () => {
  it("默认基地址为本机网关 :3000", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { ok: true } }));
    const http = new HttpCore({ fetch: impl });
    const data = await http.get<{ ok: boolean }>("/v1/iam/auth/setup-status");
    expect(data).toEqual({ ok: true });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/iam/auth/setup-status");
    expect(calls[0].init.method).toBe("GET");
  });

  it("自定义 baseUrl 去掉尾斜杠", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: {} }));
    const http = new HttpCore({ fetch: impl, baseUrl: "http://gw.example:3000/" });
    await http.get("/v1/iam/health");
    expect(calls[0].url).toBe("http://gw.example:3000/v1/iam/health");
  });

  it("query 参数跳过 undefined/null/空串", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { items: [] } }));
    const http = new HttpCore({ fetch: impl });
    await http.get("/v1/iam/users/simple", {
      params: { keyword: "ad", page: 1, size: 20, status: undefined, org_unit_id: null },
    });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/iam/users/simple?keyword=ad&page=1&size=20");
  });

  it("auth=true 注入 Bearer；token 缺失时不注入", async () => {
    let token: string | undefined = "tok-1";
    const { impl, calls } = fakeFetch(() => ({ status: 204, body: null }));
    const http = new HttpCore({ fetch: impl, getToken: () => token });

    await http.post("/v1/iam/auth/logout", undefined, { auth: true });
    expect(headersOf(calls[0])["Authorization"]).toBe("Bearer tok-1");

    token = undefined;
    await http.post("/v1/iam/auth/logout", undefined, { auth: true });
    expect(headersOf(calls[1])["Authorization"]).toBeUndefined();
  });

  it("支持异步 getToken", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: {} }));
    const http = new HttpCore({ fetch: impl, getToken: async () => "async-tok" });
    await http.get("/v1/iam/users", { auth: true });
    expect(headersOf(calls[0])["Authorization"]).toBe("Bearer async-tok");
  });

  it("auth=false（默认）不带 Authorization", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: {} }));
    const http = new HttpCore({ fetch: impl, getToken: () => "should-not-appear" });
    await http.post("/v1/iam/auth/login", { username: "a", password: "bbcccccc" });
    expect(headersOf(calls[0])["Authorization"]).toBeUndefined();
  });

  it("204 返回 undefined", async () => {
    const { impl } = fakeFetch(() => ({ status: 204, body: null }));
    const http = new HttpCore({ fetch: impl });
    const v = await http.post<void>("/v1/iam/auth/logout", undefined, { auth: true });
    expect(v).toBeUndefined();
  });

  it("非 JSON 成功体原样返回文本", async () => {
    const { impl } = fakeFetch(() => ({ status: 200, body: "plain-text", contentType: "text/plain" }));
    const http = new HttpCore({ fetch: impl });
    expect(await http.get<string>("/x")).toBe("plain-text");
  });

  it("POST 带体时设置 Content-Type: application/json", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: {} }));
    const http = new HttpCore({ fetch: impl });
    await http.post("/v1/iam/auth/login", { username: "a", password: "bbcccccc" });
    expect(headersOf(calls[0])["Content-Type"]).toBe("application/json");
    expect(calls[0].init.body).toBe(JSON.stringify({ username: "a", password: "bbcccccc" }));
  });

  it("错误响应抛 IamServicesError（纯文本形态）", async () => {
    const { impl } = fakeFetch(() => ({ status: 401, body: "invalid credentials", contentType: "text/plain" }));
    const http = new HttpCore({ fetch: impl });
    await expect(http.get("/v1/iam/users")).rejects.toMatchObject({
      name: "IamServicesError",
      status: 401,
      message: "invalid credentials",
    });
    await expect(http.get("/v1/iam/users")).rejects.toBeInstanceOf(IamServicesError);
  });

  it("错误响应抛 IamServicesError（结构化形态）", async () => {
    const { impl } = fakeFetch(() => ({ status: 400, body: { code: "VALID_2001", message: "参数错误" } }));
    const http = new HttpCore({ fetch: impl });
    await expect(http.get("/v1/iam/users")).rejects.toMatchObject({
      status: 400,
      code: "VALID_2001",
      message: "参数错误",
    });
  });
});
