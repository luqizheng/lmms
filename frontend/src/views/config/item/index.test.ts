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

  it("renders title via PureTableBar i18n key", () => {
    const wrapper = mountItem();
    const bar = wrapper.find(".pure-table-bar-stub");
    expect(bar.attributes("title")).toBe("menus.configColumnValue");
  });

  it("renders buttons via i18n keys", () => {
    const wrapper = mountItem();
    const labels = wrapper
      .findAll("button")
      .map((b: any) => b.text());
    expect(labels.some((t: string) => t.includes("menus.notificationSearch"))).toBe(true);
    expect(labels.some((t: string) => t.includes("menus.tenantRefresh"))).toBe(true);
    expect(labels.some((t: string) => t.includes("menus.configItemCreate"))).toBe(true);
  });

  it("on 搜索 click without selectedNs does not call listConfigItems", async () => {
    const wrapper = mountItem();
    const searchBtn = wrapper
      .findAll("button")
      .find((b: any) => b.text().includes("menus.notificationSearch"));
    await searchBtn.trigger("click");
    await flushPromises();
    expect(listConfigItems).not.toHaveBeenCalled();
  });
});
