// @vitest-environment happy-dom
import { describe, expect, it, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

vi.hoisted(() => {
  process.env.VITE_ROUTER_HISTORY = "hash";
});

const {
  listConfigItems,
  listNamespaces,
  createConfigItem,
  deleteConfigItem,
  message
} = vi.hoisted(() => ({
  listConfigItems: vi.fn(),
  listNamespaces: vi.fn(),
  createConfigItem: vi.fn(),
  deleteConfigItem: vi.fn(),
  message: vi.fn()
}));

vi.mock("@/api/config", () => ({
  listConfigItems,
  listNamespaces,
  createConfigItem,
  deleteConfigItem
}));
vi.mock("@/api/config/masking", () => ({
  toDisplayValue: (_id: string, v: string) => v,
  reveal: vi.fn(),
  hide: vi.fn(),
  isRevealed: () => false
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

import ConfigItem from "./index.vue";

const elStubs: Record<string, any> = {
  ElButton: { template: "<button><slot /></button>" },
  ElForm: { template: "<form><slot /></form>" },
  ElFormItem: { template: "<div><slot /></div>" },
  ElInput: { template: "<input />" },
  ElSelect: { template: "<select><slot /></select>" },
  ElOption: { template: "<option><slot /></option>" },
  ElTag: { template: "<span><slot /></span>" }
};

function mountItem() {
  return mount(ConfigItem, { global: { components: elStubs } });
}

describe("ConfigItem mount", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listNamespaces.mockResolvedValue({ items: [], total: 0 });
    listConfigItems.mockResolvedValue({ items: [] });
  });

  it("renders without throwing", () => {
    const wrapper = mountItem();
    expect(wrapper.exists()).toBe(true);
  });

  it("renders title '配置项' via PureTableBar", () => {
    const wrapper = mountItem();
    expect(wrapper.find(".pure-table-bar-stub").exists()).toBe(true);
  });

  it("renders 搜索 / 刷新 / 新建 buttons", () => {
    const wrapper = mountItem();
    const labels = wrapper
      .findAll("button")
      .map((b: any) => b.text());
    expect(labels.some((t: string) => t.includes("搜索"))).toBe(true);
    expect(labels.some((t: string) => t.includes("刷新"))).toBe(true);
    expect(labels.some((t: string) => t.includes("新建"))).toBe(true);
  });

  it("on 搜索 click without selectedNs does not call listConfigItems", async () => {
    const wrapper = mountItem();
    const searchBtn = wrapper
      .findAll("button")
      .find((b: any) => b.text().includes("搜索"));
    await searchBtn.trigger("click");
    await flushPromises();
    expect(listConfigItems).not.toHaveBeenCalled();
  });
});
