// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";

vi.mock("@/components/ReScopeSelector", () => ({
  default: { name: "ReScopeSelector", template: "<div />" }
}));
vi.mock("vue-i18n", () => ({
  useI18n: () => ({
    t: (k: string, params?: Record<string, unknown>) =>
      params ? `${k}|${JSON.stringify(params)}` : k
  })
}));

import NotificationForm from "./form.vue";

const elStubs: Record<string, any> = {
  ElForm: { template: "<form><slot /></form>" },
  ElFormItem: {
    props: ["label"],
    template: "<div><span class=\"el-form-item__label\">{{ label }}</span><slot /></div>"
  },
  ElInput: { template: "<input />" },
  ElDatePicker: { template: '<input type="datetime" />' },
  ElCheckbox: { template: "<label><slot /></label>" }
};

function mountForm(props: Record<string, unknown> = {}) {
  return mount(NotificationForm, {
    props: {
      formInline: {
        id: 0,
        title: "",
        body: "",
        publish_at: "",
        expire_at: null,
        pinned: false,
        popover: false,
        recipients: []
      },
      isEdit: false,
      ...props
    },
    global: {
      mocks: { $t: (k: string) => k },
      components: elStubs
    }
  });
}

describe("NotificationForm mount", () => {
  it("renders without throwing", () => {
    const wrapper = mountForm();
    expect(wrapper.exists()).toBe(true);
  });

  it("renders all i18n labels via $t", () => {
    const wrapper = mountForm();
    const text = wrapper.text();
    expect(text).toContain("menus.announcementTitle");
    expect(text).toContain("menus.announcementContent");
    expect(text).toContain("menus.announcementPublishTime");
    expect(text).toContain("menus.announcementExpireTime");
    expect(text).toContain("menus.announcement");
    expect(text).toContain("menus.announcementPinned");
    expect(text).toContain("menus.announcementPopover");
    expect(text).toContain("menus.announcementRecipients");
  });

  it("binds recipients prop to v-model (sync with parent)", async () => {
    const wrapper = mountForm({
      formInline: {
        id: 1,
        title: "t",
        body: "b",
        publish_at: "2026-01-01T00:00:00Z",
        expire_at: null,
        pinned: false,
        popover: false,
        recipients: [
          { scope_type: "user", scope_id: "u1", name: "alice" }
        ]
      }
    });
    const vm = wrapper.vm as unknown as { newFormInline: { recipients: unknown[] } };
    expect(vm.newFormInline.recipients).toHaveLength(1);
  });

  it("exposes getRef via defineExpose", () => {
    const wrapper = mountForm();
    const exposed = wrapper.vm.$.exposed as Record<string, unknown>;
    expect(typeof exposed.getRef).toBe("function");
  });
});