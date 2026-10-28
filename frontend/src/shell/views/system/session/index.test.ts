// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { flushPromises } from "@vue/test-utils";
import { mountShell } from "@/shell/test-utils";
import Page from "./index.vue";

const get = vi.fn();
const listUsersSimple = vi.fn();

vi.mock("@/shell/api", () => ({
  http: {
    get: (...args: unknown[]) => get(...args),
    post: vi.fn(),
    put: vi.fn()
  }
}));

vi.mock("@/shell/api/users", () => ({
  listUsersSimple: (...args: unknown[]) => listUsersSimple(...args)
}));

vi.mock("@/shell/store/auth", () => ({
  useAuthStore: () => ({ hasPermission: () => true })
}));

describe("会话管理", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    get.mockResolvedValue({ items: [], total: 0 });
    listUsersSimple.mockResolvedValue({ items: [], total: 0 });
  });

  it("加载并渲染接口返回的数据", async () => {
    get.mockResolvedValueOnce({
      items: [
        {
          id: "s1",
          user_id: "u-1",
          client_ip: "10.0.0.1",
          created_at: "2026-01-01",
          expires_at: "2026-02-01",
          is_current: true
        }
      ],
      total: 1
    });
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    expect(get).toHaveBeenCalled();
    expect(w.text()).toContain("10.0.0.1");
  });

  it("无数据时显示空态", async () => {
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    expect(w.find(".ui-data-table__empty").exists()).toBe(true);
  });

  it("用户列显示昵称而不是 userId", async () => {
    listUsersSimple.mockResolvedValue({
      items: [{ id: "u-1", nickname: "李四" }],
      total: 1,
      page: 1,
      size: 200
    });
    get.mockResolvedValueOnce({
      items: [
        {
          id: "s1",
          user_id: "u-1",
          client_ip: "10.0.0.1",
          created_at: "2026-01-01",
          expires_at: "2026-02-01",
          is_current: true
        }
      ],
      total: 1
    });
    const w = mountShell(Page);
    await flushPromises();
    await nextTick();

    expect(w.text()).toContain("李四");
    expect(w.text()).not.toContain("用户 #u-1");
  });
});
