import { describe, expect, it } from "vitest";
import { AuthApi } from "./auth.js";
import { HttpCore } from "./http.js";
import type { LoginResponse } from "./types.js";

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

const loginResponse: LoginResponse = {
  access_token: "at",
  refresh_token: "rt",
  token_type: "Bearer",
  expires_in: 900,
  user: {
    id: "usr-1",
    username: "admin",
    nickname: "管理员",
    email: null,
    phone: null,
    avatar: null,
    roles: ["admin"],
    permissions: ["IAM_USER_LIST"],
  },
};

function makeApi(impl: typeof fetch) {
  return new AuthApi(new HttpCore({ fetch: impl, getToken: () => "tok" }));
}

describe("AuthApi", () => {
  it("login 命中公开端点并透传 tenant_slug", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: loginResponse }));
    const api = makeApi(impl);
    const res = await api.login({ username: "admin", password: "secret123", tenant_slug: "acme" });
    expect(res.user.roles).toEqual(["admin"]);
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/iam/auth/login");
    expect(calls[0].init.method).toBe("POST");
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ username: "admin", password: "secret123", tenant_slug: "acme" });
    expect(headersOf(calls[0])["Authorization"]).toBeUndefined();
  });

  it("login 不带 tenant_slug 时请求体不含该键", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: loginResponse }));
    const api = makeApi(impl);
    await api.login({ username: "admin", password: "secret123" });
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ username: "admin", password: "secret123" });
  });

  it("refresh 只传 refresh_token；不轮换——响应中的 refresh_token 与传入相同", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: loginResponse }));
    const api = makeApi(impl);
    const res = await api.refresh("rt-stored");
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/iam/auth/refresh");
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ refresh_token: "rt-stored" });
    // iam 登录链路不轮换：服务端原样返回同一 refresh token，调用方不应覆盖已存值
    expect(res.refresh_token).toBe("rt");
    expect(res.user.username).toBe("admin");
  });

  it("setupStatus 为公开 GET", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { setup_required: false, user_count: 3 } }));
    const api = makeApi(impl);
    expect(await api.setupStatus()).toEqual({ setup_required: false, user_count: 3 });
    expect(headersOf(calls[0])["Authorization"]).toBeUndefined();
  });

  it("register 命中注册端点（首用户）", async () => {
    const { impl, calls } = fakeFetch(() => ({
      status: 201,
      body: { id: "usr-1", username: "admin", nickname: "管理员", status: "active" },
    }));
    const api = makeApi(impl);
    const user = await api.register({ username: "admin", password: "secret123", nickname: "管理员" });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/iam/auth/register");
    expect(user.username).toBe("admin");
  });

  it("logout 携带 Bearer 且无请求体", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 204, body: null }));
    const api = makeApi(impl);
    await api.logout();
    expect(headersOf(calls[0])["Authorization"]).toBe("Bearer tok");
    expect(calls[0].init.body).toBeUndefined();
  });

  it("changePassword 发送新旧密码", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: null }));
    const api = makeApi(impl);
    await api.changePassword({ old_password: "old12345", new_password: "new123456" });
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ old_password: "old12345", new_password: "new123456" });
  });

  it("listSessions 返回会话数组", async () => {
    const session = { id: "s1", user_id: "usr-1", client_ip: "127.0.0.1", user_agent: null, created_at: "2026-10-31T00:00:00Z", expires_at: "2026-11-30T00:00:00Z", is_current: true };
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: [session] }));
    const api = makeApi(impl);
    const sessions = await api.listSessions();
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/iam/auth/sessions");
    expect(sessions[0].is_current).toBe(true);
  });

  it("revokeSession DELETE 编码后的路径", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 204, body: null }));
    const api = makeApi(impl);
    await api.revokeSession("s/1 x");
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/iam/auth/sessions/s%2F1%20x");
    expect(calls[0].init.method).toBe("DELETE");
  });

  it("登录失败 401 纯文本归一为 IamServicesError", async () => {
    const { impl } = fakeFetch(() => ({ status: 401, body: "invalid credentials", contentType: "text/plain" }));
    const api = makeApi(impl);
    await expect(api.login({ username: "admin", password: "wrong123" })).rejects.toMatchObject({
      name: "IamServicesError",
      status: 401,
      message: "invalid credentials",
    });
  });
});
