import { describe, expect, it } from "vitest";
import { IamServicesError, parseErrorBody } from "./errors.js";

describe("parseErrorBody", () => {
  it("纯文本错误体原样作为 message（iam-service 实际行为）", () => {
    const e = parseErrorBody(401, "invalid credentials\n");
    expect(e).toEqual({ message: "invalid credentials" });
    expect(e.code).toBeUndefined();
  });

  it("空错误体回退到 HTTP 状态码文案", () => {
    expect(parseErrorBody(502, "   ").message).toBe("HTTP 502");
  });

  it("结构化 {code,message,request_id} 提取各字段", () => {
    const e = parseErrorBody(400, JSON.stringify({ code: "IAM_AUTH_1006", message: "密码强度不足", request_id: "req-1" }));
    expect(e.code).toBe("IAM_AUTH_1006");
    expect(e.message).toBe("密码强度不足");
    expect(e.requestId).toBe("req-1");
  });

  it("JSON 但无 message 字段时按纯文本处理（网关熔断体）", () => {
    const e = parseErrorBody(503, JSON.stringify({ error: "circuit_open", service: "iam-service" }));
    expect(e.message).toBe('{"error":"circuit_open","service":"iam-service"}');
  });
});

describe("IamServicesError", () => {
  it("from 保留 status 与原始文本，无 code", () => {
    const err = IamServicesError.from(401, "token expired");
    expect(err).toBeInstanceOf(Error);
    expect(err.name).toBe("IamServicesError");
    expect(err.status).toBe(401);
    expect(err.message).toBe("token expired");
    expect(err.code).toBeUndefined();
  });

  it("from 保留结构化错误体的原始 JSON", () => {
    const raw = JSON.stringify({ code: "VALID_2001", message: "参数错误" });
    const err = IamServicesError.from(400, raw);
    expect(err.code).toBe("VALID_2001");
    expect(err.body).toEqual({ code: "VALID_2001", message: "参数错误" });
  });
});
