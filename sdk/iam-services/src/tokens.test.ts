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
