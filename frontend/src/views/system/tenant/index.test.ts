// @vitest-environment happy-dom
import { describe, expect, it, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

vi.hoisted(() => {
  process.env.VITE_ROUTER_HISTORY = "hash";
});

const { http, message } = vi.hoisted(() => ({
  http: { request: vi.fn() },
  message: vi.fn()
}));

vi.mock("@/utils/http", () => ({ http }));
vi.mock("@/utils/message", () => ({ message }));
vi.mock("@/utils/sso", () => ({}));
vi.mock("@/router", () => ({}));
vi.mock("@/utils/auth", () => ({ hasPerms: () => true }));
vi.mock("@/components/RePureTableBar", () => ({
  PureTableBar: {
    name: "PureTableBar",
    template:
      '<div class="pure-table-bar-stub" :title="title"><slot /></div>',
    props: ["title"]
  }
}));
vi.mock("@/components/ReIcon/src/hooks", () => ({
  useRenderIcon: () => undefined
}));
vi.mock("vue-i18n", () => ({
  useI18n: () => ({
    t: (k: string, params?: Record<string, unknown>) =>
      params ? `${k}|${JSON.stringify(params)}` : k
  })
}));
vi.mock("@/utils/error-translate", () => ({
  translateError: (code: unknown) => `error:${code}`
}));

import SystemTenant from "./index.vue";

const elStubs: Record<string, any> = {
  ElButton: { template: "<button><slot /></button>" },
  ElForm: { template: "<form><slot /></form>" },
  ElFormItem: { template: "<div><slot /></div>" },
  ElInput: { template: "<input />" },
  ElSelect: { template: "<select><slot /></select>" },
  ElOption: { template: "<option><slot /></option>" },
  ElTable: { template: "<table><slot /></table>" },
  ElTableColumn: { template: "<td><slot /></td>" },
  ElDialog: { template: "<div><slot /></div>" },
  ElTag: { template: "<span><slot /></span>" },
  ElSwitch: { template: "<button><slot /></button>" }
};

function mountTenant() {
  return mount(SystemTenant, { global: { components: elStubs } });
}

describe("SystemTenant mount", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    http.request.mockResolvedValue([]);
  });

  it("renders without throwing", () => {
    const wrapper = mountTenant();
    expect(wrapper.exists()).toBe(true);
  });

  it("renders title via PureTableBar i18n key", () => {
    const wrapper = mountTenant();
    const bar = wrapper.find(".pure-table-bar-stub");
    expect(bar.attributes("title")).toBe("menus.systemTenant");
  });

  it("renders buttons via i18n keys", () => {
    const wrapper = mountTenant();
    const buttons = wrapper.findAll("button");
    const labels = buttons.map((b: any) => b.text());
    expect(labels.some((t: string) => t.includes("menus.tenantRefresh"))).toBe(true);
    expect(labels.some((t: string) => t.includes("menus.tenantCreate"))).toBe(true);
  });

  it("onSearch calls http.request with /v1/iam/tenants", async () => {
    const wrapper = mountTenant();
    const refreshBtn = wrapper
      .findAll("button")
      .find((b: any) => b.text().includes("menus.tenantRefresh"));
    await refreshBtn.trigger("click");
    await flushPromises();
    expect(http.request).toHaveBeenCalledWith("get", "/v1/iam/tenants");
  });

  it("onSearch on API error does not throw", async () => {
    http.request.mockRejectedValue(new Error("network"));
    const wrapper = mountTenant();
    const refreshBtn = wrapper
      .findAll("button")
      .find((b: any) => b.text().includes("menus.tenantRefresh"));
    await refreshBtn.trigger("click");
    await flushPromises();
    expect(wrapper.exists()).toBe(true);
  });
});
