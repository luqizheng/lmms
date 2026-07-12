// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

const { listSpaces } = vi.hoisted(() => ({
  listSpaces: vi.fn(async () => ({
    data: { items: [], total: 0, page: 1, size: 20 }
  }))
}));

vi.mock("@/api/kb", () => ({
  listSpaces,
  createSpace: vi.fn(async () => ({ data: { id: 1 } })),
  getSpace: vi.fn(),
  updateSpace: vi.fn(),
  deleteSpace: vi.fn()
}));

vi.mock("@/utils/auth", () => ({
  getToken: () => "",
  formatToken: () => ""
}));

import PmKbSpaces from "./spaces-list.vue";

const elStubs: Record<string, any> = {
  ElButton: { template: '<button class="el-button-stub"><slot /></button>' },
  ElTable: {
    template:
      '<div class="el-table-stub"><slot :data="[]" /></div>'
  },
  ElTableColumn: {
    template: '<div class="el-table-column-stub"><slot /></div>',
    props: ["prop", "label", "width", "minWidth", "showOverflowTooltip", "fixed"]
  },
  ElTag: { template: '<span class="el-tag-stub"><slot /></span>' },
  ElEmpty: { template: '<div class="el-empty-stub"><slot /></div>' },
  ElPagination: { template: '<div class="el-pagination-stub" />' },
  ElDialog: { template: '<div class="el-dialog-stub"><slot /></div>' },
  ElForm: { template: "<form><slot /></form>" },
  ElFormItem: {
    props: ["label"],
    template:
      '<div><span class="el-form-item__label">{{ label }}</span><slot /></div>'
  },
  ElInput: { template: "<input />" },
  ElRadioGroup: { template: '<div><slot /></div>' },
  ElRadioButton: { template: '<label><slot /></label>' }
};

function mountSpaces() {
  return mount(PmKbSpaces, {
    global: {
      stubs: elStubs,
      mocks: { $t: (k: string) => k }
    }
  });
}

describe("PmKbSpaces", () => {
  it("renders list and loads spaces on mount", async () => {
    const wrapper = mountSpaces();
    await flushPromises();
    expect(listSpaces).toHaveBeenCalledTimes(1);
    expect(wrapper.find(".pm-kb-spaces").exists()).toBe(true);
    expect(wrapper.find(".el-table-stub").exists()).toBe(true);
  });

  it("opens create dialog when user clicks 新建空间", async () => {
    const wrapper = mountSpaces();
    await flushPromises();
    const buttons = wrapper.findAll(".el-button-stub");
    const createBtn = buttons.find(b => b.text().includes("新建空间"));
    expect(createBtn).toBeTruthy();
    await createBtn!.trigger("click");
    await flushPromises();
    expect(wrapper.find(".el-dialog-stub").exists()).toBe(true);
  });
});
