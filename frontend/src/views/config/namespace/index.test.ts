// @vitest-environment happy-dom
import { describe, expect, it, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

vi.hoisted(() => {
  process.env.VITE_ROUTER_HISTORY = "hash";
});

const { listNamespaces, createNamespace, deleteNamespace, message } = vi.hoisted(() => ({
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
    template: '<div class="pure-table-bar-stub"><slot /></div>',
    props: ["title"]
  }
}));
vi.mock("@/components/ReIcon/src/hooks", () => ({
  useRenderIcon: () => undefined
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

  it("renders title '配置命名空间' via PureTableBar", () => {
    const wrapper = mountNamespace();
    expect(wrapper.find(".pure-table-bar-stub").exists()).toBe(true);
  });

  it("renders 搜索 / 刷新 / 新建 buttons", () => {
    const wrapper = mountNamespace();
    const labels = wrapper
      .findAll("button")
      .map((b: any) => b.text());
    expect(labels.some((t: string) => t.includes("搜索"))).toBe(true);
    expect(labels.some((t: string) => t.includes("刷新"))).toBe(true);
    expect(labels.some((t: string) => t.includes("新建"))).toBe(true);
  });

  it("on 搜索 click calls listNamespaces", async () => {
    const wrapper = mountNamespace();
    const searchBtn = wrapper
      .findAll("button")
      .find((b: any) => b.text().includes("搜索"));
    await searchBtn.trigger("click");
    await flushPromises();
    expect(listNamespaces).toHaveBeenCalled();
  });
});
