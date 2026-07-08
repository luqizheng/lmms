import { describe, expect, it, vi, beforeEach } from "vitest";

const {
  mockAxiosInstance,
  handRefreshToken,
  logOut,
  getToken,
  message,
  $t,
  transformI18n
} = vi.hoisted(() => {
  const mockAxiosInstance: any = {
    interceptors: {
      request: { use: vi.fn() },
      response: { use: vi.fn() }
    },
    request: vi.fn()
  };
  return {
    mockAxiosInstance,
    handRefreshToken: vi.fn(),
    logOut: vi.fn(),
    getToken: vi.fn(),
    message: vi.fn(),
    $t: vi.fn((k: string) => k),
    transformI18n: vi.fn((k: string) => k)
  };
});

vi.mock("axios", () => {
  const isCancel = vi.fn((e: any) => e?.__isCancel === true);
  return {
    default: {
      create: () => mockAxiosInstance,
      isCancel
    }
  };
});

vi.mock("@/store/modules/user", () => ({
  useUserStoreHook: () => ({ handRefreshToken, logOut })
}));

vi.mock("@/utils/auth", () => ({
  getToken,
  formatToken: (t: string) => "Bearer " + t
}));

vi.mock("@/utils/message", () => ({ message }));

vi.mock("@/plugins/i18n", () => ({ $t, transformI18n }));

vi.mock("@/utils/sso", () => ({}));

let http: any;

const applyRequestInterceptors = async (config: any) => {
  for (const cb of mockAxiosInstance.interceptors.request.use.mock.calls) {
    config = await cb[0](config);
  }
  return config;
};

const runSuccessInterceptor = (response: any) => {
  for (const cb of mockAxiosInstance.interceptors.response.use.mock.calls) {
    return cb[0](response);
  }
  return response;
};

const runErrorInterceptor = (error: any) => {
  for (const cb of mockAxiosInstance.interceptors.response.use.mock.calls) {
    return cb[1](error);
  }
  return Promise.reject(error);
};

const defaultSuccessImpl = async (config: any) => {
  await applyRequestInterceptors(config);
  return runSuccessInterceptor({ data: { ok: true }, config: {} });
};

beforeEach(async () => {
  vi.clearAllMocks();
  mockAxiosInstance.request.mockReset();
  mockAxiosInstance.interceptors.request.use.mockClear();
  mockAxiosInstance.interceptors.response.use.mockClear();
  getToken.mockReset();
  handRefreshToken.mockReset();
  logOut.mockReset();
  message.mockReset();
  vi.resetModules();
  const mod = await import("./index");
  http = mod.http;
  mockAxiosInstance.request.mockImplementation(defaultSuccessImpl);
});

describe("http.request basic flow", () => {
  it("resolves with response.data on success", async () => {
    const out = await http.request("get", "/v1/iam/users");
    expect(out).toEqual({ ok: true });
    expect(mockAxiosInstance.request).toHaveBeenCalledTimes(1);
  });

  it("marks isCancelRequest on cancel errors", async () => {
    mockAxiosInstance.request.mockImplementationOnce(async (config: any) => {
      await applyRequestInterceptors(config);
      return runErrorInterceptor({ __isCancel: true });
    });
    await expect(http.request("get", "/x")).rejects.toMatchObject({
      isCancelRequest: true
    });
  });

  it("http.get / http.post pass method correctly", async () => {
    await http.get("/v1/iam/users");
    expect(mockAxiosInstance.request).toHaveBeenLastCalledWith(
      expect.objectContaining({ method: "get", url: "/v1/iam/users" })
    );
    await http.post("/v1/iam/users", { data: { name: "x" } });
    expect(mockAxiosInstance.request).toHaveBeenLastCalledWith(
      expect.objectContaining({ method: "post", url: "/v1/iam/users" })
    );
  });
});

describe("request interceptor: token handling", () => {
  it("skips Authorization header for whitelist URLs", async () => {
    const cfg = { url: "/auth/login", headers: {} };
    const out = await applyRequestInterceptors(cfg);
    expect(out.headers.Authorization).toBeUndefined();
    expect(getToken).not.toHaveBeenCalled();
  });

  it("attaches Authorization when token is valid (not expired)", async () => {
    getToken.mockReturnValue({
      accessToken: "abc",
      refreshToken: "rt-1",
      expires: Date.now() + 60_000
    });
    const cfg = { url: "/v1/iam/users", headers: {} };
    const out = await applyRequestInterceptors(cfg);
    expect(out.headers.Authorization).toBe("Bearer abc");
  });

  it("resolves without Authorization when no token in storage", async () => {
    getToken.mockReturnValue(null);
    const cfg = { url: "/v1/iam/users", headers: {} };
    const out = await applyRequestInterceptors(cfg);
    expect(out.headers.Authorization).toBeUndefined();
  });

  it("triggers refresh flow when token expired; success path updates header", async () => {
    getToken.mockReturnValue({
      accessToken: "old",
      refreshToken: "rt-1",
      expires: Date.now() - 1000
    });
    handRefreshToken.mockResolvedValue({ access_token: "new-tok" });

    const cfg = { url: "/v1/iam/users", headers: {} };
    const p = applyRequestInterceptors(cfg);
    await new Promise(r => setTimeout(r, 5));
    await p;
    expect(handRefreshToken).toHaveBeenCalledWith({ refreshToken: "rt-1" });
  });

  it("on refresh failure, calls logOut and shows expired message", async () => {
    getToken.mockReturnValue({
      accessToken: "old",
      refreshToken: "rt-1",
      expires: Date.now() - 1000
    });
    handRefreshToken.mockRejectedValue(new Error("refresh fail"));

    const cfg = { url: "/v1/iam/users", headers: {} };
    await expect(applyRequestInterceptors(cfg)).rejects.toThrow(
      "refresh token failed"
    );
    await new Promise(r => setTimeout(r, 5));
    expect(logOut).toHaveBeenCalled();
    expect(message).toHaveBeenCalled();
  });
});

describe("response interceptor", () => {
  it("returns response.data on success", () => {
    const out = runSuccessInterceptor({ data: { foo: 1 }, config: {} });
    expect(out).toEqual({ foo: 1 });
  });

  it("rejects with isCancelRequest flag on cancel", async () => {
    const err = { __isCancel: true };
    await expect(runErrorInterceptor(err)).rejects.toMatchObject({
      isCancelRequest: true
    });
  });
});
