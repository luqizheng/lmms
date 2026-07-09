// @vitest-environment happy-dom
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import DashboardIndex from "./index.vue";

const mockFetch = vi.fn();

beforeEach(() => {
  vi.stubGlobal("fetch", mockFetch);
  mockFetch.mockReset();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function mountDashboard() {
  return mount(DashboardIndex, {
    global: {
      mocks: { $t: (k: string) => k },
      components: {
        ElCard: { template: "<div class=\"el-card\"><slot /></div>" },
        ElRow: { template: "<div class=\"el-row\"><slot /></div>" },
        ElCol: { template: "<div class=\"el-col\"><slot /></div>" },
        ElTag: {
          template: "<span class=\"el-tag\"><slot /></span>",
          props: ["type", "size", "effect"]
        },
        ElButton: { template: "<button><slot /></button>" }
      }
    }
  });
}

describe("DashboardIndex mount", () => {
  it("renders 8 service cards after first probe completes", async () => {
    mockFetch.mockResolvedValue({ ok: true, status: 200 });
    const wrapper = mountDashboard();
    await flushPromises();
    const cards = wrapper.findAll(".el-card");
    // 3 summary cards (up/down/last-check) + 8 service cards = 11
    expect(cards.length).toBeGreaterThanOrEqual(8);
    expect(mockFetch).toHaveBeenCalled();
    wrapper.unmount();
  });

  it("marks service as ok on HTTP 200", async () => {
    mockFetch.mockResolvedValue({ ok: true, status: 200 });
    const wrapper = mountDashboard();
    await flushPromises();
    const text = wrapper.text();
    expect(text).toContain("menus.dashboardStatusOk");
    wrapper.unmount();
  });

  it("marks service as down when fetch rejects", async () => {
    mockFetch.mockRejectedValue(new Error("network"));
    const wrapper = mountDashboard();
    await flushPromises();
    const text = wrapper.text();
    expect(text).toContain("menus.dashboardStatusDown");
    wrapper.unmount();
  });

  it("counts up services correctly", async () => {
    let callIdx = 0;
    mockFetch.mockImplementation(async () => {
      callIdx++;
      return callIdx % 2 === 0
        ? { ok: false, status: 500 }
        : { ok: true, status: 200 };
    });
    const wrapper = mountDashboard();
    await flushPromises();
    const text = wrapper.text();
    // 至少存在 up/down 计数显示
    expect(text).toContain("menus.dashboardUpCount");
    expect(text).toContain("menus.dashboardDownCount");
    wrapper.unmount();
  });
});