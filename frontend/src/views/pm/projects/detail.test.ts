// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

const { listRequirements, createRequirement, deleteRequirement, getProject } =
  vi.hoisted(() => ({
    listRequirements: vi.fn(async () => ({
      data: { items: [], total: 0, page: 1, size: 20 }
    })),
    createRequirement: vi.fn(async () => ({ data: { id: 1 } })),
    deleteRequirement: vi.fn(async () => ({ data: undefined })),
    getProject: vi.fn(async () => ({
      data: { id: 42, code: "ACME", name: "Acme", status: "active" }
    }))
  }));

const pmStoreState: { currentProject: any } = vi.hoisted(() => ({
  currentProject: null as any
}));

vi.mock("vue-router", () => ({
  useRouter: () => ({ push: vi.fn(), back: vi.fn() }),
  useRoute: () => ({ params: { id: "42" } })
}));

vi.mock("@/api/pm", () => ({
  listRequirements,
  createRequirement,
  deleteRequirement,
  getProject,
  listProjects: vi.fn(),
  createProject: vi.fn(),
  archiveProject: vi.fn(),
  updateProject: vi.fn(),
  getRequirement: vi.fn(),
  updateRequirement: vi.fn(),
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

vi.mock("@/store/modules/pm", () => ({
  usePmStore: () => ({
    get currentProject() {
      return pmStoreState.currentProject;
    },
    set currentProject(v: any) {
      pmStoreState.currentProject = v;
    },
    loadProject: vi.fn(async (id: number) => {
      // delegate to mocked @/api/pm so tests can assert on getProject calls
      const res = await getProject(id);
      pmStoreState.currentProject = res.data;
      return pmStoreState.currentProject;
    }),
    loadRequirements: vi.fn(async (projId: number) => {
      const res = await listRequirements(projId, {});
      return res.data;
    })
  })
}));

import PmProjectDetail from "./detail.vue";

const elStubs: Record<string, any> = {
  ElPageHeader: {
    name: "ElPageHeader",
    template:
      '<div class="el-page-header-stub" @click="$emit(\'back\')"><slot name="content" /><slot name="extra" /></div>'
  },
  ElCard: { template: '<div class="el-card-stub"><slot /><slot name="header" /></div>' },
  ElSelect: {
    props: ["modelValue", "placeholder"],
    template:
      '<select class="el-select-stub" :value="modelValue" @change="$emit(\x27update:modelValue\x27, $event.target.value)"><slot /></select>'
  },
  ElOption: { props: ["label", "value"], template: '<option :value="value">{{ label }}</option>' },
  ElButton: { template: '<button class="el-button-stub"><slot /></button>' },
  ElTable: { template: '<div class="el-table-stub"><slot /></div>' },
  ElTableColumn: {
    template: '<div class="el-table-column-stub"><slot /></div>',
    props: ["prop", "label", "width", "minWidth", "showOverflowTooltip", "fixed"]
  },
  ElTag: { props: ["type", "size"], template: '<span class="el-tag-stub"><slot /></span>' },
  ElPagination: { template: '<div class="el-pagination-stub" />' },
  ElDialog: { template: '<div class="el-dialog-stub"><slot /></div>' },
  ElForm: { template: "<form><slot /></form>" },
  ElFormItem: {
    props: ["label"],
    template:
      '<div><span class="el-form-item__label">{{ label }}</span><slot /></div>'
  },
  ElInput: { template: "<input />" },
  ElInputNumber: { props: ["min"], template: '<input type="number" />' },
  ElRadioGroup: { template: '<div><slot /></div>' },
  ElRadioButton: { props: ["value"], template: '<label><slot /></label>' }
};

function mountDetail() {
  return mount(PmProjectDetail, {
    global: {
      stubs: elStubs,
      mocks: { $t: (k: string) => k }
    }
  });
}

describe("PmProjectDetail", () => {
  it("loads the project and the requirements list on mount", async () => {
    const wrapper = mountDetail();
    await flushPromises();
    expect(getProject).toHaveBeenCalledWith(42);
    expect(listRequirements).toHaveBeenCalled();
    expect(wrapper.find(".pm-project-detail").exists()).toBe(true);
  });

  it("opens new-requirement dialog when clicking 新建需求", async () => {
    const wrapper = mountDetail();
    await flushPromises();
    const buttons = wrapper.findAll(".el-button-stub");
    const createBtn = buttons.find(b => b.text().includes("新建需求"));
    expect(createBtn).toBeTruthy();
    await createBtn!.trigger("click");
    await flushPromises();
    expect(wrapper.find(".el-dialog-stub").exists()).toBe(true);
  });
});
