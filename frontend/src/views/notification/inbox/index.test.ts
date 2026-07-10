// @vitest-environment happy-dom
import { describe, expect, it, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

vi.hoisted(() => {
  process.env.VITE_ROUTER_HISTORY = "hash";
});

const { useInbox, markRead } = vi.hoisted(() => ({
  useInbox: vi.fn(),
  markRead: vi.fn()
}));

vi.mock("./utils/hook", () => ({ useInbox }));
vi.mock("@/api/notification", () => ({ markRead }));
vi.mock("@/store/modules/notification", () => ({
  useNotificationStore: () => ({
    readOne: vi.fn(),
    readAll: vi.fn().mockResolvedValue(undefined)
  })
}));
vi.mock("@/utils/sso", () => ({}));
vi.mock("@/router", () => ({}));
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
vi.mock("vue-i18n", () => ({
  useI18n: () => ({
    t: (k: string, params?: Record<string, unknown>) =>
      params ? `${k}|${JSON.stringify(params)}` : k
  }),
  createI18n: () => ({}),
  default: {}
}));

import NotificationInbox from "./index.vue";

const elStubs: Record<string, any> = {
  ElButton: { template: "<button><slot /></button>" },
  ElForm: { template: "<form><slot /></form>" },
  ElFormItem: {
    props: ["label"],
    template: "<div><span class=\"el-form-item__label\">{{ label }}</span><slot /></div>"
  },
  ElInput: { template: "<input />" },
  ElTabs: { template: "<div><slot /></div>" },
  ElTabPane: {
    props: ["label"],
    template: "<div class=\"el-tab-pane-stub\">{{ label }}<slot /></div>"
  },
  ElDrawer: { template: "<div class=\"el-drawer-stub\"><slot /></div>" },
  ElTag: { template: "<span class=\"el-tag\"><slot /></span>" }
};

function defaultInboxApi() {
  return {
    form: { keyword: "" },
    loading: { value: false },
    columns: [],
    dataList: { value: [] },
    activeTab: "all",
    unreadCount: { value: 0 },
    onSearch: vi.fn(),
    onTabChange: vi.fn(),
    openDetail: vi.fn()
  };
}

function mountInbox() {
  return mount(NotificationInbox, { global: { components: elStubs } });
}

describe("NotificationInbox mount", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useInbox.mockReturnValue(defaultInboxApi());
  });

  it("renders without throwing", () => {
    const wrapper = mountInbox();
    expect(wrapper.exists()).toBe(true);
  });

  it("renders title '通知收件箱' via PureTableBar", () => {
    const wrapper = mountInbox();
    expect(wrapper.find(".pure-table-bar-stub").exists()).toBe(true);
  });

it("renders 搜索 and 重置 buttons via i18n key", () => {
    const wrapper = mountInbox();
    const labels = wrapper
      .findAll("button")
      .map((b: any) => b.text());
    expect(labels.some((t: string) => t.includes("menus.notificationSearch"))).toBe(true);
    expect(labels.some((t: string) => t.includes("menus.notificationReset"))).toBe(true);
  });

  it("hides 全部标为已读 button when unreadCount is 0", () => {
    const wrapper = mountInbox();
    const labels = wrapper
      .findAll("button")
      .map((b: any) => b.text());
    expect(
      labels.some((t: string) => t.includes("menus.notificationMarkAllRead"))
    ).toBe(false);
  });

  it("opens detail drawer on row-click and auto-marks read", async () => {
    useInbox.mockReturnValue({
      ...defaultInboxApi(),
      dataList: {
        value: [
          { id: 42, title: "测试公告", body: "正文", pinned: false, popover: false }
        ]
      }
    });
    const wrapper = mountInbox();
    await flushPromises();
    const vm = wrapper.vm as unknown as {
      openDrawer: (row: unknown) => Promise<void>;
    };
    await vm.openDrawer({ id: 42, title: "测试公告", body: "正文" });
    await flushPromises();
    expect(markRead).toHaveBeenCalledWith(42);
  });

  it("calls useInbox hook on mount", () => {
    mountInbox();
    expect(useInbox).toHaveBeenCalledTimes(1);
  });
});
