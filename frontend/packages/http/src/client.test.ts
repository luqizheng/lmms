// @vitest-environment node
import { describe, expect, it, vi } from "vitest";
import axios, { type AxiosAdapter, type InternalAxiosRequestConfig } from "axios";
import { createHttp, HttpError } from "./index";

interface Recorded {
  url?: string;
  method?: string;
  authorization?: string;
  data?: unknown;
  params?: unknown;
}

function adapterFactory(
  responses: Array<{ status: number; data: unknown }>
): { adapter: AxiosAdapter; calls: Recorded[] } {
  const calls: Recorded[] = [];
  let index = 0;
  const adapter: AxiosAdapter = async config => {
    calls.push({
      url: config.url,
      method: config.method,
      authorization: config.headers?.get("Authorization") as string | undefined,
      data: config.data,
      params: config.params
    });
    const response = responses[Math.min(index, responses.length - 1)];
    index += 1;
    if (response.status >= 400) {
      const error = new axios.AxiosError(
        "Request failed",
        "ERR_BAD_REQUEST",
        config as InternalAxiosRequestConfig,
        undefined,
        {
          data: response.data,
          status: response.status,
          statusText: "Error",
          headers: {},
          config: config as InternalAxiosRequestConfig
        }
      );
      throw error;
    }
    return {
      data: response.data,
      status: response.status,
      statusText: "OK",
      headers: {},
      config: config as InternalAxiosRequestConfig
    };
  };
  return { adapter, calls };
}

function makeClient(
  responses: Array<{ status: number; data: unknown }>,
  options: Parameters<typeof createHttp>[0] = {}
) {
  const { adapter, calls } = adapterFactory(responses);
  const instance = axios.create({ adapter });
  const client = createHttp({ ...options, instance });
  return { client, calls };
}

describe("createHttp", () => {
  it("成功时返回响应体本身", async () => {
    const { client } = makeClient([{ status: 200, data: { ok: true } }]);
    await expect(client.get("/v1/iam/users")).resolves.toEqual({ ok: true });
  });

  it("携带 Authorization 头", async () => {
    const { client, calls } = makeClient([{ status: 200, data: {} }], {
      getToken: () => "token-abc"
    });
    await client.get("/v1/iam/users");
    expect(calls[0].authorization).toBe("Bearer token-abc");
  });

  it("白名单路径不带 Authorization", async () => {
    const { client, calls } = makeClient([{ status: 200, data: {} }], {
      getToken: () => "token-abc",
      authWhiteList: ["/v1/iam/auth/login"]
    });
    await client.post("/v1/iam/auth/login", { username: "a" });
    expect(calls[0].authorization).toBeUndefined();
  });

  it("GET 把参数放在 query，POST 放在 body", async () => {
    const { client, calls } = makeClient([
      { status: 200, data: {} },
      { status: 200, data: {} }
    ]);
    await client.get("/v1/users", { page: 2 });
    await client.post("/v1/users", { name: "x" });
    expect(calls[0].params).toEqual({ page: 2 });
    expect(calls[0].data).toBeUndefined();
    expect(calls[1].data).toBe(JSON.stringify({ name: "x" }));
  });

  it("后端业务错误抛出 HttpError，并带错误码", async () => {
    const { client } = makeClient([
      { status: 400, data: { code: "IAM_USER_CREATE", message: "user exists" } }
    ]);
    await expect(client.post("/v1/iam/users", {})).rejects.toBeInstanceOf(HttpError);
  });

  it("错误文案可由消费方翻译", async () => {
    const { client } = makeClient(
      [{ status: 400, data: { code: "IAM_USER_CREATE", message: "user exists" } }],
      { translateError: code => (code === "IAM_USER_CREATE" ? "用户已存在" : undefined) }
    );
    await expect(client.post("/v1/iam/users", {})).rejects.toMatchObject({
      code: "IAM_USER_CREATE",
      message: "用户已存在"
    });
  });

  it("401 时刷新 token 并重放原请求", async () => {
    const refreshToken = vi.fn().mockResolvedValue("new-token");
    const { client, calls } = makeClient(
      [
        { status: 401, data: { code: "IAM_AUTH_1001", message: "expired" } },
        { status: 200, data: { ok: true } }
      ],
      { getToken: () => "old-token", refreshToken }
    );

    await expect(client.get("/v1/iam/users")).resolves.toEqual({ ok: true });
    expect(refreshToken).toHaveBeenCalledTimes(1);
    expect(calls.length).toBe(2);
    expect(calls[1].authorization).toBe("Bearer new-token");
  });

  it("刷新失败时调用 onUnauthorized 并抛出 HttpError", async () => {
    const onUnauthorized = vi.fn();
    const { client } = makeClient(
      [{ status: 401, data: { code: "IAM_AUTH_1001", message: "expired" } }],
      {
        getToken: () => "old-token",
        refreshToken: async () => undefined,
        onUnauthorized
      }
    );

    await expect(client.get("/v1/iam/users")).rejects.toBeInstanceOf(HttpError);
    expect(onUnauthorized).toHaveBeenCalledTimes(1);
  });

  it("并发 401 只触发一次刷新", async () => {
    let resolveRefresh: ((token: string) => void) | undefined;
    const refreshToken = vi.fn(
      () =>
        new Promise<string>(resolve => {
          resolveRefresh = resolve;
        })
    );
    const { client, calls } = makeClient(
      [
        { status: 401, data: { code: "IAM_AUTH_1001", message: "expired" } },
        { status: 401, data: { code: "IAM_AUTH_1001", message: "expired" } },
        { status: 200, data: { ok: true } },
        { status: 200, data: { ok: true } }
      ],
      { getToken: () => "old-token", refreshToken }
    );

    const first = client.get("/v1/a");
    const second = client.get("/v1/b");
    await new Promise(resolve => setTimeout(resolve, 0));
    resolveRefresh?.("shared-token");
    await Promise.all([first, second]);

    expect(refreshToken).toHaveBeenCalledTimes(1);
    expect(calls.filter(call => call.authorization === "Bearer shared-token").length).toBe(2);
  });

  it("skipAuth 的请求不参与刷新重放", async () => {
    const refreshToken = vi.fn().mockResolvedValue("new-token");
    const { client } = makeClient(
      [{ status: 401, data: { code: "IAM_AUTH_1001", message: "expired" } }],
      { getToken: () => "old-token", refreshToken }
    );
    await expect(
      client.get("/v1/public", undefined, { skipAuth: true })
    ).rejects.toBeInstanceOf(HttpError);
    expect(refreshToken).not.toHaveBeenCalled();
  });

  it("持续 401 只刷新重放一次，不无限刷新", async () => {
    const refreshToken = vi.fn().mockResolvedValue("new-token");
    const { client } = makeClient(
      [{ status: 401, data: { code: "IAM_AUTH_1001", message: "expired" } }],
      { getToken: () => "old-token", refreshToken }
    );

    await expect(client.get("/v1/iam/users")).rejects.toBeInstanceOf(HttpError);
    expect(refreshToken).toHaveBeenCalledTimes(1);
  });

  it("刷新端点自身 401 不会让客户端死锁", async () => {
    // 场景：业务请求 401 → 触发刷新 → 刷新请求自己也 401
    // 期望：刷新请求不参与刷新链路，原始请求以 HttpError 结束而不是永久挂起
    const calls: string[] = [];
    const refreshToken = vi.fn(async () => {
      calls.push("refresh");
      // 用同一个 client 发刷新请求，它会 401
      try {
        await clientRef.post("/v1/iam/auth/refresh", { refresh_token: "r" }, { skipAuth: true });
      } catch {
        return undefined;
      }
      return undefined;
    });

    const { adapter } = adapterFactory([
      { status: 401, data: { code: "IAM_AUTH_1001", message: "expired" } },
      { status: 401, data: { code: "IAM_AUTH_1001", message: "expired" } }
    ]);
    const instance = axios.create({ adapter });
    const clientRef = createHttp({
      instance,
      getToken: () => "old-token",
      refreshToken
    });

    await expect(
      Promise.race([
        clientRef.get("/v1/iam/users").catch((e) => e),
        new Promise(resolve => setTimeout(() => resolve("TIMEOUT"), 2000))
      ])
    ).resolves.toBeInstanceOf(HttpError);
    expect(calls.length).toBeGreaterThan(0);
  });

  it("403 不触发 token 刷新", async () => {
    const refreshToken = vi.fn().mockResolvedValue("new-token");
    const { client } = makeClient(
      [{ status: 403, data: { code: "IAM_AUTH_1003", message: "forbidden" } }],
      { getToken: () => "old-token", refreshToken }
    );
    await expect(client.get("/v1/iam/users")).rejects.toBeInstanceOf(HttpError);
    expect(refreshToken).not.toHaveBeenCalled();
  });

  it("白名单路径带 query string 也不带 token", async () => {
    const { client, calls } = makeClient([{ status: 200, data: {} }], {
      getToken: () => "token-abc",
      authWhiteList: ["/v1/iam/auth/login"]
    });
    await client.get("/v1/iam/auth/login?next=%2Fdashboard");
    expect(calls[0].authorization).toBeUndefined();
  });

  it("并发失败时所有请求都拿到 HttpError", async () => {
    const { client } = makeClient(
      [
        { status: 401, data: { code: "IAM_AUTH_1001", message: "expired" } },
        { status: 401, data: { code: "IAM_AUTH_1001", message: "expired" } },
        { status: 401, data: { code: "IAM_AUTH_1001", message: "expired" } }
      ],
      { getToken: () => "old-token", refreshToken: async () => undefined }
    );

    const results = await Promise.allSettled([
      client.get("/v1/a"),
      client.get("/v1/b")
    ]);
    for (const result of results) {
      expect(result.status).toBe("rejected");
      if (result.status === "rejected") {
        expect(result.reason).toBeInstanceOf(HttpError);
      }
    }
  });
});
