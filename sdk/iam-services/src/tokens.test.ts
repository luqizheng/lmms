import { describe, expect, it } from "vitest";
import { HttpCore } from "./http.js";
import { TokensApi } from "./tokens.js";

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

function makeApi(impl: typeof fetch) {
  return new TokensApi(new HttpCore({ fetch: impl, getToken: () => "svc-tok" }));
}

describe("TokensApi", () => {
  it("issueAccess client_credentials 请求体不含 code", async () => {
    const { impl, calls } = fakeFetch(() => ({
      status: 200,
      body: { access_token: "at", refresh_token: "", token_type: "Bearer", expires_in: 900 },
    }));
    const tokens = makeApi(impl);
    await tokens.issueAccess({ grant_type: "client_credentials", client_id: "app", client_secret: "s3cret" });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/tokens/access");
    expect(JSON.parse(String(calls[0].init.body))).toEqual({
      grant_type: "client_credentials",
      client_id: "app",
      client_secret: "s3cret",
    });
    expect(headersOf(calls[0])["Authorization"]).toBeUndefined();
  });

  it("issueAccess authorization_code 携带 code", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { access_token: "at", refresh_token: "rt", token_type: "Bearer", expires_in: 900 } }));
    const tokens = makeApi(impl);
    await tokens.issueAccess({ grant_type: "authorization_code", client_id: "app", client_secret: "s3cret", code: "abc" });
    expect(JSON.parse(String(calls[0].init.body))).toEqual({
      grant_type: "authorization_code",
      client_id: "app",
      client_secret: "s3cret",
      code: "abc",
    });
  });

  it("refresh（OAuth 链路）需要 client 凭证三件套", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { access_token: "at", refresh_token: "rt-new", token_type: "Bearer", expires_in: 900 } }));
    const tokens = makeApi(impl);
    const res = await tokens.refresh({ refresh_token: "rt-old", client_id: "app", client_secret: "s3cret" });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/tokens/refresh");
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ refresh_token: "rt-old", client_id: "app", client_secret: "s3cret" });
    // OAuth 链路带轮换：新 refresh token 应被采用（与 iam 链路相反）
    expect(res.refresh_token).toBe("rt-new");
  });

  it("revoke 发送 token_jti 与 expires_in", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { success: true } }));
    const tokens = makeApi(impl);
    const res = await tokens.revoke({ token_jti: "jti-1", expires_in: 300 });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/tokens/revoke");
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ token_jti: "jti-1", expires_in: 300 });
    expect(res.success).toBe(true);
  });

  it("jwks 命中公开 JWKS 端点", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { keys: [{ kty: "RSA", kid: "default" }] } }));
    const tokens = makeApi(impl);
    const jwks = await tokens.jwks();
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/tokens/.well-known/jwks.json");
    expect(jwks.keys[0].kid).toBe("default");
  });

  it("apiKeys.list 携带 Bearer", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { items: [], total: 0, page: 1, size: 20 } }));
    const tokens = makeApi(impl);
    await tokens.apiKeys.list();
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/api-keys");
    expect(headersOf(calls[0])["Authorization"]).toBe("Bearer svc-tok");
  });

  it("apiKeys.create 透传创建请求体与一次性 secret 响应", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { api_key_id: "k1", api_key_secret: "one-time-secret" } }));
    const tokens = makeApi(impl);
    const res = await tokens.apiKeys.create({ name: "pm-worker", permissions: ["pm:read"] });
    expect(calls[0].init.method).toBe("POST");
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ name: "pm-worker", permissions: ["pm:read"] });
    expect(res.api_key_secret).toBe("one-time-secret");
  });
});

describe("ServiceJwtGrantsApi", () => {
  it("list 发送过滤与分页参数并带鉴权头", async () => {
    const { impl, calls } = fakeFetch(() => ({
      status: 200,
      body: {
        items: [
          { owner: "svc-a", audience: "iam-service", status: "active", system_managed: false },
        ],
        total: 1,
        page: 1,
        size: 20,
      },
    }));
    const tokens = makeApi(impl);
    const page = await tokens.serviceJwtGrants.list({ owner: "svc-a", status: "active", page: 1, size: 20 });
    expect(page.total).toBe(1);
    expect(page.items[0].audience).toBe("iam-service");
    const url = calls[0].url;
    expect(url).toContain("/v1/admin/service-jwt-grants?");
    expect(url).toContain("owner=svc-a");
    expect(url).toContain("status=active");
    expect(headersOf(calls[0])["Authorization"]).toBe("Bearer svc-tok");
  });

  it("create POST body 正确（重复创建服务端 409）", async () => {
    const { impl, calls } = fakeFetch(() => ({
      status: 200,
      body: { owner: "svc-a", audience: "file-service", status: "active", system_managed: false },
    }));
    const tokens = makeApi(impl);
    const g = await tokens.serviceJwtGrants.create({ owner: "svc-a", audience: "file-service" });
    expect(g.audience).toBe("file-service");
    expect(calls[0].url.endsWith("/v1/admin/service-jwt-grants")).toBe(true);
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ owner: "svc-a", audience: "file-service" });
    expect(calls[0].init.method).toBe("POST");
  });

  it("setStatus 走 PUT 路径，remove 走 DELETE 路径", async () => {
    const { impl, calls } = fakeFetch(() => ({
      status: 200,
      body: { owner: "svc-a", audience: "file-service", status: "disabled", system_managed: false },
    }));
    const tokens = makeApi(impl);
    const g = await tokens.serviceJwtGrants.setStatus("svc-a", "file-service", { status: "disabled" });
    expect(g.status).toBe("disabled");
    expect(calls[0].init.method).toBe("PUT");
    expect(calls[0].url).toContain("/v1/admin/service-jwt-grants/svc-a/file-service/status");

    // 204 delete
    const { impl: impl2, calls: calls2 } = fakeFetch(() => ({ status: 204, body: undefined }));
    const tokens2 = makeApi(impl2);
    await tokens2.serviceJwtGrants.remove("svc-a", "file-service");
    expect(calls2[0].init.method).toBe("DELETE");
    expect(calls2[0].url.endsWith("/v1/admin/service-jwt-grants/svc-a/file-service")).toBe(true);
  });
});
