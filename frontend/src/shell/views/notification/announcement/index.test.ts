// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import Page from "./index.vue";

const get = vi.fn();

vi.mock("@/shell/api", () => ({
  http: {
    get: (...args: unknown[]) => get(...args),
    post: vi.fn(),
    put: vi.fn()
  }
}));

vi.mock("@/shell/store/auth", () => ({
  useAuthStore: () => ({ hasPermission: () => true })
}));

describe("公告管理", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    get.mockResolvedValue({ items: [], total: 0 });
  });

  it("加载并渲染接口返回的数据", async () => {
    get.mockResolvedValueOnce({
      items: [
        {
          id: 1,
          title: "系统升级通知",
          body: "b",
          publish_at: "2026-01-01",
          status: "published",
          pinned: false
        }
      ],
      total: 1
    });
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    expect(get).toHaveBeenCalled();
    expect(w.text()).toContain("系统升级通知");
  });

  it("无数据时显示空态", async () => {
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    expect(w.find(".ui-data-table__empty").exists()).toBe(true);
  });
});
