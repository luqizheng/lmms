// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import Page from "./index.vue";

const listMenuConfigs = vi.fn();

vi.mock("@/shell/api", () => ({
  listMenuConfigs: (...args: unknown[]) => listMenuConfigs(...args),
  publishMenuConfig: vi.fn(),
  cloneMenuConfig: vi.fn()
}));

vi.mock("@/shell/store/auth", () => ({
  useAuthStore: () => ({ hasPermission: () => true })
}));

describe("菜单管理", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listMenuConfigs.mockResolvedValue({ items: [], total: 0 });
  });

  it("加载并渲染接口返回的数据", async () => {
    listMenuConfigs.mockResolvedValueOnce({
      items: [
        {
          id: "1",
          name: "系统管理",
          version: 3,
          is_active: true,
          created_at: "2026-01-01"
        }
      ],
      total: 1
    });
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    expect(listMenuConfigs).toHaveBeenCalled();
    expect(w.text()).toContain("系统管理");
  });

  it("无数据时显示空态", async () => {
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    expect(w.find(".ui-data-table__empty").exists()).toBe(true);
  });
});
