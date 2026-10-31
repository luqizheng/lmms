import { describe, expect, it } from "vitest";
import { AuditServicesError } from "./errors.js";
import { createAuditServices } from "./index.js";

interface RecordedCall {
  url: string;
  init: RequestInit;
}

function fakeFetch(responder: (call: RecordedCall) => { status: number; body: unknown; contentType?: string; headers?: Record<string, string> }) {
  const calls: RecordedCall[] = [];
  const impl = (async (input: string | URL, init?: RequestInit) => {
    const call: RecordedCall = { url: String(input), init: init ?? {} };
    calls.push(call);
    const res = responder(call);
    const text =
      typeof res.body === "string" ? res.body : res.body === undefined || res.body === null ? "" : JSON.stringify(res.body);
    return new Response(res.status === 204 ? null : text, {
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
  return createAuditServices({ fetch: impl, getToken: () => "tok" });
}

describe("AuditLogsApi", () => {
  it("queryLogin 组装查询参数并携带 Bearer", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { items: [], total: 0, page: 1, size: 20 } }));
    const sdk = makeSdk(impl);
    const res = await sdk.logs.queryLogin({ username: "admin", success: false, page: 2, size: 10 });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/audit/login?username=admin&success=false&page=2&size=10");
    expect(headersOf(calls[0])["Authorization"]).toBe("Bearer tok");
    expect(res.total).toBe(0);
  });

  it("queryOperation 透传过滤条件", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { items: [], total: 0, page: 1, size: 20 } }));
    const sdk = makeSdk(impl);
    await sdk.logs.queryOperation({ user_id: "u1", action: "update", resource: "project" });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/audit/operation?user_id=u1&action=update&resource=project");
  });
});

describe("AuditExportsApi", () => {
  it("loginCsv 以文本模式返回 CSV（text/csv）", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: "id,username\n1,admin", contentType: "text/csv; charset=utf-8" }));
    const sdk = makeSdk(impl);
    const csv = await sdk.exports.loginCsv("u/1");
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/audit/export/compliance/login/csv/u%2F1");
    expect(csv).toBe("id,username\n1,admin");
  });

  it("operationJson 解析裸数组", async () => {
    const log = { id: 1, user_id: "u1", action: "a", resource: "r", request_id: "q", created_at: "2026-10-31T00:00:00Z" };
    const { impl } = fakeFetch(() => ({ status: 200, body: [log] }));
    const sdk = makeSdk(impl);
    const arr = await sdk.exports.operationJson("u1");
    expect(arr).toHaveLength(1);
    expect(arr[0].action).toBe("a");
  });
});

describe("AuditRetentionApi", () => {
  it("updatePolicy PUT 完整策略体", async () => {
    const policy = { hot_days: 7, warm_days: 30, cold_days: 90, archive_enabled: true, purge_enabled: false };
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: policy }));
    const sdk = makeSdk(impl);
    const res = await sdk.retention.updatePolicy(policy);
    expect(calls[0].init.method).toBe("PUT");
    expect(JSON.parse(String(calls[0].init.body))).toEqual(policy);
    expect(res.cold_days).toBe(90);
  });

  it("cleanup POST 返回实际处理器形状", async () => {
    const { impl, calls } = fakeFetch(() => ({
      status: 200,
      body: { deleted_count: 5, cutoff_date: "2026-10-01T00:00:00Z", success: true },
    }));
    const sdk = makeSdk(impl);
    const res = await sdk.retention.cleanup();
    expect(calls[0].init.method).toBe("POST");
    expect(res.deleted_count).toBe(5);
    expect(res.success).toBe(true);
  });

  it("dataStage 对 RFC3339 路径参数编码", async () => {
    const { impl, calls } = fakeFetch(() => ({
      status: 200,
      body: { created_at: "2026-10-31T00:00:00Z", stage: "Hot", policy: { hot_days: 7, warm_days: 30, cold_days: 90 } },
    }));
    const sdk = makeSdk(impl);
    const res = await sdk.retention.dataStage("2026-10-31T00:00:00Z");
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/audit/retention/stage/2026-10-31T00%3A00%3A00Z");
    expect(res.stage).toBe("Hot");
  });
});

describe("错误归一", () => {
  it("结构化 {code,message,request_id} 提取各字段（audit 走 common-error）", async () => {
    const { impl } = fakeFetch(() => ({
      status: 400,
      body: { code: "CFG_VALID_2003", message: "保留天数必须递增", request_id: "req-9" },
    }));
    const sdk = makeSdk(impl);
    const err = await sdk.retention.updatePolicy({ hot_days: 90, warm_days: 30, cold_days: 7, archive_enabled: true, purge_enabled: false }).catch((e) => e);
    expect(err).toBeInstanceOf(AuditServicesError);
    expect(err.status).toBe(400);
    expect(err.code).toBe("CFG_VALID_2003");
    expect(err.requestId).toBe("req-9");
  });

  it("纯文本错误体（网关层）原样作为 message", async () => {
    const { impl } = fakeFetch(() => ({ status: 401, body: "Invalid or expired token", contentType: "text/plain" }));
    const sdk = makeSdk(impl);
    await expect(sdk.logs.queryLogin()).rejects.toMatchObject({ status: 401, message: "Invalid or expired token" });
  });
});
