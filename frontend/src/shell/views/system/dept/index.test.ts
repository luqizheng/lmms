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

describe("组织架构", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    get.mockResolvedValue([]);
  });

  it("加载并渲染组织单元树", async () => {
    get.mockResolvedValueOnce([
      {
        id: "1",
        name: "总部",
        code: "HQ",
        level: 1,
        sort_order: 0,
        status: "enabled",
        created_at: "2026-01-01",
        children: [
          {
            id: "2",
            name: "研发部",
            code: "RD",
            level: 2,
            sort_order: 0,
            status: "enabled",
            created_at: "2026-01-01",
            children: []
          }
        ]
      }
    ]);

    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    expect(get).toHaveBeenCalledWith("/v1/iam/org-units");
    expect(w.text()).toContain("总部");
    expect(w.text()).toContain("研发部");
  });
});
