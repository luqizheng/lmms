import { describe, expect, it } from "vitest";
import { HttpCore } from "./http.js";
import { OrgUnitsApi } from "./org-units.js";
import { PermissionsApi } from "./permissions.js";
import { RolesApi } from "./roles.js";
import { TokensApi } from "./tokens.js";
import { UsersApi } from "./users.js";

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

function makeApi<T>(Ctor: new (http: HttpCore) => T, impl: typeof fetch): T {
  return new Ctor(new HttpCore({ fetch: impl, getToken: () => "tok" }));
}

describe("UsersApi", () => {
  it("page 默认无查询串且携带 Bearer", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { items: [], total: 0, page: 1, size: 20 } }));
    const users = makeApi(UsersApi, impl);
    const res = await users.page();
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/iam/users");
    expect(headersOf(calls[0])["Authorization"]).toBe("Bearer tok");
    expect(res.total).toBe(0);
  });

  it("page 透传 keyword/status/org_unit_id/分页", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { items: [], total: 0, page: 2, size: 10 } }));
    const users = makeApi(UsersApi, impl);
    await users.page({ keyword: "张", status: "active", org_unit_id: "org-1", page: 2, size: 10 });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/iam/users?keyword=%E5%BC%A0&status=active&org_unit_id=org-1&page=2&size=10");
  });

  it("pageSimple 组装 keyword/page/size", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { items: [{ id: "u1", nickname: "张三" }], total: 1, page: 1, size: 20 } }));
    const users = makeApi(UsersApi, impl);
    const res = await users.pageSimple({ keyword: "张", page: 1, size: 20 });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/iam/users/simple?keyword=%E5%BC%A0&page=1&size=20");
    expect(res.items[0].nickname).toBe("张三");
  });

  it("get/update/remove 使用编码后的路径", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { id: "u/1" } }));
    const users = makeApi(UsersApi, impl);
    await users.get("u/1");
    await users.update("u/1", { nickname: "新名" });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/iam/users/u%2F1");
    expect(calls[1].init.method).toBe("PUT");
    expect(JSON.parse(String(calls[1].init.body))).toEqual({ nickname: "新名" });
  });

  it("remove 返回 void（204）", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 204, body: null }));
    const users = makeApi(UsersApi, impl);
    expect(await users.remove("u1")).toBeUndefined();
    expect(calls[0].init.method).toBe("DELETE");
  });

  it("create 发送创建请求体", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { id: "u2" } }));
    const users = makeApi(UsersApi, impl);
    await users.create({ username: "alice", password: "password123", nickname: "爱丽丝", role_ids: ["r1"] });
    expect(calls[0].init.method).toBe("POST");
    expect(JSON.parse(String(calls[0].init.body))).toEqual({
      username: "alice",
      password: "password123",
      nickname: "爱丽丝",
      role_ids: ["r1"],
    });
  });

  it("assignRoles 请求体为 {role_ids}", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { user_id: "u1", role_ids: ["r1", "r2"] } }));
    const users = makeApi(UsersApi, impl);
    const res = await users.assignRoles("u1", ["r1", "r2"]);
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/iam/users/u1/roles");
    expect(calls[0].init.method).toBe("PUT");
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ role_ids: ["r1", "r2"] });
    expect(res.role_ids).toEqual(["r1", "r2"]);
  });
});

describe("PermissionsApi", () => {
  it("page 组装 keyword/category 分页", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { items: [{ id: "p1", code: "IAM_USER_READ", name: "用户查看", description: null, category: "1", resource: "user", action: "read", sort_order: 1, status: "active", created_at: "", updated_at: "" }], total: 1, page: 1, size: 20 } }));
    const perms = makeApi(PermissionsApi, impl);
    const res = await perms.page({ keyword: "用户", category: 1, page: 1, size: 20 });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/iam/permissions?keyword=%E7%94%A8%E6%88%B7&category=1&page=1&size=20");
    expect(res.items[0].category).toBe("1"); // 出参为字符串
  });

  it("create 入参 category 为数字", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { id: "p2" } }));
    const perms = makeApi(PermissionsApi, impl);
    await perms.create({ code: "PM_LIST", name: "项目查看", category: 9 });
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ code: "PM_LIST", name: "项目查看", category: 9 });
  });

  it("update PUT / remove DELETE 204", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 204, body: null }));
    const perms = makeApi(PermissionsApi, impl);
    await perms.update("p1", { name: "新名" });
    expect(calls[0].init.method).toBe("PUT");
    expect(await perms.remove("p1")).toBeUndefined();
    expect(calls[1].init.method).toBe("DELETE");
  });
});

describe("OrgUnitsApi", () => {
  it("list 返回部门树（裸数组）", async () => {
    const tree = [{ id: "org-1", parent_id: null, name: "总公司", code: "HQ", level: 1, sort_order: 1, status: "active", created_at: "", updated_at: "", children: [{ id: "org-2", parent_id: "org-1", name: "研发部", code: "RD", level: 2, sort_order: 1, status: "active", created_at: "", updated_at: "", children: [] }] }];
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: tree }));
    const orgs = makeApi(OrgUnitsApi, impl);
    const res = await orgs.list();
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/iam/org-units");
    expect(res[0].children[0].code).toBe("RD");
  });

  it("create POST / assignUsers PUT {user_ids} 204", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 204, body: null }));
    const orgs = makeApi(OrgUnitsApi, impl);
    await orgs.create({ parent_id: "org-1", name: "测试部", code: "QA" });
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ parent_id: "org-1", name: "测试部", code: "QA" });
    await orgs.assignUsers("org-2", ["usr-1", "usr-2"]);
    expect(calls[1].url).toBe("http://127.0.0.1:3000/v1/iam/org-units/org-2/users");
    expect(JSON.parse(String(calls[1].init.body))).toEqual({ user_ids: ["usr-1", "usr-2"] });
  });
});

describe("ApiKeysApi.list", () => {
  it("透传 keyword/page/size 查询参数", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { items: [], total: 0, page: 1, size: 10 } }));
    const tokens = makeApi(TokensApi, impl);
    await tokens.apiKeys.list({ keyword: "pm", page: 1, size: 10 });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/api-keys?keyword=pm&page=1&size=10");
  });
});

describe("RolesApi", () => {
  it("list 分页查询角色", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { items: [{ id: "r1", name: "管理员", code: "admin", description: null, data_scope: "1", status: "active", created_at: "", updated_at: "" }], total: 1, page: 1, size: 20 } }));
    const roles = makeApi(RolesApi, impl);
    const res = await roles.list({ page: 1, size: 20 });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/iam/roles?page=1&size=20");
    expect(res.items[0].code).toBe("admin");
  });

  it("get 单角色路径编码", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { id: "r1" } }));
    const roles = makeApi(RolesApi, impl);
    await roles.get("r/1");
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/iam/roles/r%2F1");
  });

  it("create POST 完整请求体（data_scope 必填数字）", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { id: "r2", name: "访客", code: "guest", description: null, data_scope: "1", status: "active", created_at: "", updated_at: "" } }));
    const roles = makeApi(RolesApi, impl);
    const res = await roles.create({ name: "访客", code: "guest", data_scope: 1 });
    expect(calls[0].init.method).toBe("POST");
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ name: "访客", code: "guest", data_scope: 1 });
    expect(res.id).toBe("r2");
  });

  it("update PUT 部分字段", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { id: "r1" } }));
    const roles = makeApi(RolesApi, impl);
    await roles.update("r1", { name: "新名", description: "d" });
    expect(calls[0].init.method).toBe("PUT");
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ name: "新名", description: "d" });
  });

  it("remove DELETE 204", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 204, body: null }));
    const roles = makeApi(RolesApi, impl);
    expect(await roles.remove("r1")).toBeUndefined();
    expect(calls[0].init.method).toBe("DELETE");
  });
});
