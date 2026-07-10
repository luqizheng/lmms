// @vitest-environment happy-dom
import { describe, expect, it, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

const { onSearch, openDialog, handlePublish, handleRevoke } = vi.hoisted(
  () => ({
    onSearch: vi.fn(),
    openDialog: vi.fn(),
    handlePublish: vi.fn(),
    handleRevoke: vi.fn()
  })
);

vi.mock("@/views/notification/admin/utils/hook", () => ({
  useAdmin: () => ({
    form: { keyword: "", status: "" },
    loading: false,
    columns: [],
    dataList: [],
    onSearch,
    openDialog,
    handlePublish,
    handleRevoke
  })
}));
vi.mock("@/utils/sso", () => ({}));
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

import NotificationAdmin from "./index.vue";

const elStubs: Record<string, any> = {
  ElForm: { template: "<form><slot /></form>" },
  ElFormItem: {
    props: ["label"],
    template: "<div><span class=\"el-form-item__label\">{{ label }}</span><slot /></div>"
  },
  ElInput: { template: "<input />" },
  ElSelect: { template: "<select><slot /></slot>" },
  ElOption: { template: "<option><slot /></option>" },
  ElButton: { template: "<button><slot /></button>" }
};

function mountAdmin() {
  return mount(NotificationAdmin, {
    global: {
      mocks: { $t: (k: string) => k },
      components: elStubs
    }
  });
}

describe("NotificationAdmin mount", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders without throwing", () => {
    const wrapper = mountAdmin();
    expect(wrapper.exists()).toBe(true);
  });

  it("renders all i18n keys (form labels + buttons + placeholders + options)", () => {
    const wrapper = mountAdmin();
    const html = wrapper.html();
    const text = wrapper.text();
    expect(text).toContain("menus.notificationTitle");
    expect(text).toContain("menus.notificationStatus");
    expect(html).toContain("menus.notificationFilterTitle");
    expect(html).toContain("menus.notificationFilterAll");
    expect(html).toContain("menus.notificationDraft");
    expect(html).toContain("menus.notificationPublished");
    expect(html).toContain("menus.notificationRevoked");
    expect(text).toContain("menus.notificationSearch");
    expect(text).toContain("menus.notificationReset");
  });

  it("renders title via PureTableBar i18n key", () => {
    const wrapper = mountAdmin();
    const bar = wrapper.find(".pure-table-bar-stub");
    expect(bar.attributes("title")).toBe("menus.notificationAdmin");
  });

  it("click search button invokes onSearch", async () => {
    const wrapper = mountAdmin();
    const btns = wrapper.findAll("button");
    const searchBtn = btns.find((b: any) =>
      b.text().includes("menus.notificationSearch")
    );
    await searchBtn.trigger("click");
    expect(onSearch).toHaveBeenCalled();
  });
});