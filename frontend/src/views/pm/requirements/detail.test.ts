// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

const { getRequirement, listStories, listCriteria, decomposeRequirement, refineRequirement } =
  vi.hoisted(() => ({
    getRequirement: vi.fn(async () => ({
      data: {
        id: 99,
        title: "Test Requirement",
        type: "feature",
        status: "open",
        priority: "p2"
      }
    })),
    listStories: vi.fn(async () => ({ data: [] })),
    listCriteria: vi.fn(async () => ({ data: [] })),
    decomposeRequirement: vi.fn(async () => ({
      data: { provider: "noop", content: "AI not configured", finishReason: "disabled", disabled: true }
    })),
    refineRequirement: vi.fn(async () => ({
      data: { provider: "noop", content: "AI not configured", finishReason: "disabled", disabled: true }
    }))
  }));

const pmStoreState: {
  currentRequirement: any;
  stories: any[];
  criteria: any[];
  versions: any[];
} = vi.hoisted(() => ({
  // Pre-populate so the v-if="pmStore.currentRequirement" template guard
  // renders the .pm-req-detail root on mount.
  currentRequirement: {
    id: 99,
    title: "Test Requirement",
    type: "feature",
    status: "open",
    priority: "p2"
  } as any,
  stories: [],
  criteria: [],
  versions: []
}));

vi.mock("vue-router", () => ({
  useRouter: () => ({ push: vi.fn(), back: vi.fn() }),
  useRoute: () => ({ params: { id: "99" } })
}));

vi.mock("@/api/pm", () => ({
  getRequirement,
  listStories,
  listCriteria,
  listProjects: vi.fn(),
  createProject: vi.fn(),
  archiveProject: vi.fn(),
  updateProject: vi.fn(),
  getProject: vi.fn(),
  listRequirements: vi.fn(),
  createRequirement: vi.fn(),
  updateRequirement: vi.fn(),
  deleteRequirement: vi.fn(),
  createStory: vi.fn(),
  updateStory: vi.fn(),
  createCriterion: vi.fn(),
  updateCriterion: vi.fn(),
  checkCriterion: vi.fn()
}));

vi.mock("@/api/llm", () => ({
  decomposeRequirement: (...args: any[]) => (decomposeRequirement as any)(...args),
  refineRequirement: (...args: any[]) => (refineRequirement as any)(...args),
  listProviders: vi.fn(),
  parseDecomposedJson: vi.fn(() => null)
}));

vi.mock("@/utils/auth", () => ({
  getToken: () => "",
  formatToken: () => ""
}));

vi.mock("@/store/modules/pm", () => ({
  usePmStore: () => ({
    get currentRequirement() {
      return pmStoreState.currentRequirement;
    },
    set currentRequirement(v: any) {
      pmStoreState.currentRequirement = v;
    },
    get stories() {
      return pmStoreState.stories;
    },
    set stories(v: any[]) {
      pmStoreState.stories = v;
    },
    get criteria() {
      return pmStoreState.criteria;
    },
    set criteria(v: any[]) {
      pmStoreState.criteria = v;
    },
    get versions() {
      return pmStoreState.versions;
    },
    set versions(v: any[]) {
      pmStoreState.versions = v;
    },
    loadRequirement: vi.fn(async (id: number) => {
      const res = await getRequirement(id);
      pmStoreState.currentRequirement = res.data;
      return pmStoreState.currentRequirement;
    }),
    loadStories: vi.fn(async (reqId: number) => {
      const res = await listStories(reqId);
      return res.data;
    }),
    loadCriteria: vi.fn(async () => []),
    loadDocument: vi.fn(),
    loadVersions: vi.fn(),
    clearAll: vi.fn()
  })
}));

// Element Plus icons are used directly; stub them to keep test light.
vi.mock("@element-plus/icons-vue", () => ({
  ArrowLeft: { template: "<span />" },
  Loading: { template: "<span />" }
}));

import PmRequirementDetail from "./detail.vue";

const elStubs: Record<string, any> = {
  ElPageHeader: {
    name: "ElPageHeader",
    props: ["icon"],
    template:
      '<div class="el-page-header-stub" @click="$emit(\'back\')"><slot name="content" /><slot name="extra" /></div>'
  },
  ElCard: { template: '<div class="el-card-stub"><slot /><slot name="header" /></div>' },
  ElButton: {
    props: ["disabled", "loading", "type"],
    template: '<button class="el-button-stub" :disabled="disabled"><slot /></button>'
  },
  ElTag: { props: ["type", "size"], template: '<span class="el-tag-stub"><slot /></span>' },
  ElEmpty: { template: '<div class="el-empty-stub"><slot /></div>' },
  ElSelect: {
    props: ["modelValue"],
    template:
      '<select class="el-select-stub" :value="modelValue" @change="$emit(\'update:modelValue\', $event.target.value)"><slot /></select>'
  },
  ElOption: { props: ["label", "value"], template: '<option :value="value">{{ label }}</option>' },
  ElCheckbox: {
    props: ["modelValue"],
    template:
      '<label class="el-checkbox-stub"><input type="checkbox" :checked="!!modelValue" @change="$emit(\'update:modelValue\', $event.target.checked)" /><slot /></label>'
  },
  ElDialog: {
    props: ["modelValue", "title", "width"],
    template:
      '<div v-if="modelValue" class="el-dialog-stub"><slot /><slot name="footer" /></div>'
  },
  ElForm: { template: "<form><slot /></form>" },
  ElFormItem: {
    props: ["label"],
    template: '<div><span class="el-form-item__label">{{ label }}</span><slot /></div>'
  },
  ElInput: { template: "<input />", props: ["modelValue"] },
  ElInputNumber: { props: ["min"], template: '<input type="number" />' },
  ElTextarea: { props: ["rows"], template: '<textarea />' },
  ElAlert: {
    props: ["type", "title", "closable", "showIcon", "description"],
    template: '<div class="el-alert-stub" :data-type="type"><slot /></div>'
  },
  ElIcon: { template: "<i><slot /></i>" },
  ElMessage: { error: vi.fn(), success: vi.fn(), warning: vi.fn() }
};

// Stub ElMessage static methods so we don't get runtime errors.
vi.mock("element-plus", async () => {
  const actual = await vi.importActual("element-plus");
  return {
    ...(actual as any),
    ElMessage: { error: vi.fn(), success: vi.fn(), warning: vi.fn() }
  };
});

function mountDetail() {
  return mount(PmRequirementDetail, {
    global: {
      stubs: elStubs,
      mocks: { $t: (k: string) => k }
    }
  });
}

describe("PmRequirementDetail", () => {
  it("loads requirement and stories on mount", async () => {
    const wrapper = mountDetail();
    await flushPromises();
    expect(getRequirement).toHaveBeenCalledWith(99);
    expect(listStories).toHaveBeenCalled();
    expect(wrapper.find(".pm-req-detail").exists()).toBe(true);
  });

  it("renders enabled AI 拆解 and AI 改写 buttons (DeepSeek wiring done)", async () => {
    const wrapper = mountDetail();
    await flushPromises();
    const buttons = wrapper.findAll(".el-button-stub");
    const decompose = buttons.find(b => b.text().includes("AI 拆解"));
    const refine = buttons.find(b => b.text().includes("AI 改写"));
    expect(decompose).toBeTruthy();
    expect(refine).toBeTruthy();
    // Buttons are enabled — clicks drive LLM API and open dialog.
    expect((decompose!.element as HTMLButtonElement).disabled).toBe(false);
    expect((refine!.element as HTMLButtonElement).disabled).toBe(false);
  });

  it("opens create-story dialog when clicking 添加故事", async () => {
    const wrapper = mountDetail();
    await flushPromises();
    const buttons = wrapper.findAll(".el-button-stub");
    const createStoryBtn = buttons.find(b => b.text().includes("添加故事"));
    expect(createStoryBtn).toBeTruthy();
    await createStoryBtn!.trigger("click");
    await flushPromises();
    expect(wrapper.find(".el-dialog-stub").exists()).toBe(true);
  });

  it("AI 拆解 opens dialog and shows disabled alert when provider is noop", async () => {
    const wrapper = mountDetail();
    await flushPromises();
    const buttons = wrapper.findAll(".el-button-stub");
    const decompose = buttons.find(b => b.text().includes("AI 拆解"))!;
    await decompose.trigger("click");
    await flushPromises();
    // Dialog opens with disabled alert rendered.
    const dialog = wrapper.find(".el-dialog-stub");
    expect(dialog.exists()).toBe(true);
    expect(dialog.find(".el-alert-stub").exists()).toBe(true);
    expect(decomposeRequirement).toHaveBeenCalledWith(99);
  });
});
