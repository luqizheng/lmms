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

describe("登录日志", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    get.mockResolvedValue({ items: [], total: 0 });
  });

  it("加载并渲染接口返回的数据", async () => {
    get.mockResolvedValueOnce({
      items: [
        {
          username: "admin",
          ip: "10.0.0.1",
          success: true,
          created_at: "2026-01-01"
        }
      ],
      total: 1
    });
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    expect(get).toHaveBeenCalled();
    expect(w.text()).toContain("admin");
  });

  it("无数据时显示空态", async () => {
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    expect(w.find(".ui-data-table__empty").exists()).toBe(true);
  });
});
