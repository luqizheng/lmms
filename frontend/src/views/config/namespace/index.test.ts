// @vitest-environment happy-dom
import { describe, expect, it, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

vi.hoisted(() => {
  process.env.VITE_ROUTER_HISTORY = "hash";
});

const { listNamespaces, createNamespace, deleteNamespace, message } =
  vi.hoisted(() => ({
    listNamespaces: vi.fn(),
    createNamespace: vi.fn(),
    deleteNamespace: vi.fn(),
    message: vi.fn()
  }));

vi.mock("@/api/config", () => ({
  listNamespaces,
  createNamespace,
  deleteNamespace
}));
vi.mock("@/utils/message", () => ({ message }));
vi.mock("@/utils/sso", () => ({}));
vi.mock("@/router", () => ({}));
vi.mock("@/utils/auth", () => ({ hasPerms: () => true }));
vi.mock("@/components/RePureTableBar", () => ({
  PureTableBar: {
    name: "PureTableBar",
    template: '<div class="pure-table-bar-stub" :title="title"><slot /></div>',
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

import ConfigNamespace from "./index.vue";

const elStubs: Record<string, any> = {
  ElButton: { template: "<button><slot /></button>" },
  ElForm: { template: "<form><slot /></form>" },
  ElFormItem: { template: "<div><slot /></div>" },
  ElInput: { template: "<input />" },
  ElTag: { template: "<span><slot /></span>" }
};

function mountNamespace() {
  return mount(ConfigNamespace, { global: { components: elStubs } });
}

describe("ConfigNamespace mount", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listNamespaces.mockResolvedValue({ items: [], total: 0 });
  });

  it("renders without throwing", () => {
    const wrapper = mountNamespace();
    expect(wrapper.exists()).toBe(true);
  });

  it("renders title via PureTableBar i18n key", () => {
    const wrapper = mountNamespace();
    const bar = wrapper.find(".pure-table-bar-stub");
    expect(bar.attributes("title")).toBe("menus.configColumnApp");
  });

  it("renders buttons via i18n keys", () => {
    const wrapper = mountNamespace();
    const labels = wrapper.findAll("button").map((b: any) => b.text());
    expect(
      labels.some((t: string) => t.includes("menus.notificationSearch"))
    ).toBe(true);
    expect(labels.some((t: string) => t.includes("menus.tenantRefresh"))).toBe(
      true
    );
    expect(
      labels.some((t: string) => t.includes("menus.configNamespaceCreate"))
    ).toBe(true);
  });

  it("on 搜索 click calls listNamespaces", async () => {
    const wrapper = mountNamespace();
    const searchBtn = wrapper
      .findAll("button")
      .find((b: any) => b.text().includes("menus.notificationSearch"));
    await searchBtn.trigger("click");
    await flushPromises();
    expect(listNamespaces).toHaveBeenCalled();
  });
});
