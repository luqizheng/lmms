import { describe, expect, it } from "vitest";
import { NotificationServicesError } from "./errors.js";
import { createNotificationServices } from "./index.js";

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
  return createNotificationServices({ fetch: impl, getToken: () => "tok" });
}

describe("NotificationAnnouncementsApi", () => {
  it("listMine 返回裸数组并透传 pinned/active", async () => {
    const ann = { id: 1, title: "t", body: "b", publish_at: "2026-10-31T00:00:00Z", pinned: 1, popover: 0, status: "published", created_by: "admin", created_at: "", expire_at: null };
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: [ann] }));
    const sdk = makeSdk(impl);
    const list = await sdk.announcements.listMine({ pinned: true });
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/notification/announcements?pinned=true");
    expect(headersOf(calls[0])["Authorization"]).toBe("Bearer tok");
    expect(list[0].pinned).toBe(1); // 响应里是 0/1 整数
  });

  it("create 发送 recipients（入参为 boolean），201 返回 {id}", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 201, body: { id: 7 } }));
    const sdk = makeSdk(impl);
    const res = await sdk.announcements.create({
      title: "t", body: "b", publish_at: "2026-10-31T00:00:00Z",
      pinned: true, popover: false,
      recipients: [{ scope_type: "role", scope_id: "admin" }],
    });
    expect(calls[0].init.method).toBe("POST");
    expect(JSON.parse(String(calls[0].init.body))).toEqual({
      title: "t", body: "b", publish_at: "2026-10-31T00:00:00Z",
      pinned: true, popover: false,
      recipients: [{ scope_type: "role", scope_id: "admin" }],
    });
    expect(res.id).toBe(7);
  });

  it("publish / revoke / markRead 命中对应路径", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { id: 1, status: "published", user_count: 3 } }));
    const sdk = makeSdk(impl);
    const pub = await sdk.announcements.publish(1);
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/notification/announcements/1/publish");
    expect(pub.user_count).toBe(3);
    await sdk.announcements.revoke(1);
    expect(calls[1].url).toBe("http://127.0.0.1:3000/v1/notification/announcements/1/revoke");
    await sdk.announcements.markRead(1);
    expect(calls[2].url).toBe("http://127.0.0.1:3000/v1/notification/announcements/1/read");
  });
});

describe("NotificationUnreadApi", () => {
  it("unread.count()", async () => {
    const { impl, calls } = fakeFetch(() => ({ status: 200, body: { count: 5 } }));
    const sdk = makeSdk(impl);
    expect((await sdk.unread.count()).count).toBe(5);
    expect(calls[0].url).toBe("http://127.0.0.1:3000/v1/notification/unread-count");
  });
});

describe("错误归一（notification 形态）", () => {
  it("{error} 形态提取 message", async () => {
    const { impl } = fakeFetch(() => ({ status: 403, body: { error: "admin only" } }));
    const sdk = makeSdk(impl);
    const err = await sdk.announcements.create({
      title: "t", body: "b", publish_at: "x", pinned: false, popover: false, recipients: [],
    }).catch((e) => e);
    expect(err).toBeInstanceOf(NotificationServicesError);
    expect(err.status).toBe(403);
    expect(err.message).toBe("admin only");
  });

  it("{error:'upstream',code,message} 提取 code 与内层 message", async () => {
    const { impl } = fakeFetch(() => ({ status: 502, body: { error: "upstream", code: "NTF_DB_9001", message: "db down" } }));
    const sdk = makeSdk(impl);
    const err = await sdk.unread.count().catch((e) => e);
    expect(err.code).toBe("NTF_DB_9001");
    expect(err.message).toBe("db down");
  });
});
