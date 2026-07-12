// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

const { listProjects, createProject, archiveProject } = vi.hoisted(() => ({
  listProjects: vi.fn(async () => ({
    data: { items: [], total: 0, page: 1, size: 20 }
  })),
  createProject: vi.fn(async () => ({ data: { id: 1 } })),
  archiveProject: vi.fn(async () => ({ data: undefined }))
}));

const push = vi.fn();
vi.mock("vue-router", () => ({
  useRouter: () => ({ push }),
  useRoute: () => ({ params: {} })
}));

vi.mock("@/api/pm", () => ({
  listProjects,
  createProject,
  archiveProject,
  getProject: vi.fn(),
  listRequirements: vi.fn(),
  getRequirement: vi.fn(),
  createRequirement: vi.fn(),
  updateRequirement: vi.fn(),
  deleteRequirement: vi.fn(),
  listStories: vi.fn(),
  createStory: vi.fn(),
  updateStory: vi.fn(),
  listCriteria: vi.fn(),
  createCriterion: vi.fn(),
  updateCriterion: vi.fn(),
  checkCriterion: vi.fn()
}));

vi.mock("@/utils/auth", () => ({
  getToken: () => "",
  formatToken: () => ""
}));

import PmProjectList from "./list.vue";

const elStubs: Record<string, any> = {
  ElInput: { template: "<input />" },
  ElButton: { template: '<button class="el-button-stub"><slot /></button>' },
  ElTable: { template: '<div class="el-table-stub"><slot /></div>' },
  ElTableColumn: {
    template: '<div class="el-table-column-stub"><slot /></div>',
    props: ["prop", "label", "width", "minWidth", "showOverflowTooltip", "fixed"]
  },
  ElTag: {
    props: ["type", "size"],
    template: '<span class="el-tag-stub"><slot /></span>'
  },
  ElPagination: { template: '<div class="el-pagination-stub" />' },
  ElDialog: { template: '<div class="el-dialog-stub"><slot /></div>' },
  ElForm: { template: "<form><slot /></form>" },
  ElFormItem: {
    props: ["label"],
    template:
      '<div><span class="el-form-item__label">{{ label }}</span><slot /></div>'
  },
  ElTextarea: { template: '<textarea />' }
};

function mountList() {
  return mount(PmProjectList, {
    global: {
      stubs: elStubs,
      mocks: { $t: (k: string) => k }
    }
  });
}

describe("PmProjectList", () => {
  it("mounts and loads projects", async () => {
    const wrapper = mountList();
    await flushPromises();
    expect(listProjects).toHaveBeenCalledTimes(1);
    expect(wrapper.find(".pm-projects").exists()).toBe(true);
  });

  it("opens new-project dialog when user clicks 新建项目", async () => {
    const wrapper = mountList();
    await flushPromises();
    const buttons = wrapper.findAll(".el-button-stub");
    const createBtn = buttons.find(b => b.text().includes("新建项目"));
    expect(createBtn).toBeTruthy();
    await createBtn!.trigger("click");
    await flushPromises();
    expect(wrapper.find(".el-dialog-stub").exists()).toBe(true);
  });
});
