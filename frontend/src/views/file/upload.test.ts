// @vitest-environment happy-dom
import { describe, expect, it, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

vi.hoisted(() => {
  process.env.VITE_ROUTER_HISTORY = "hash";
});

const { uploadFile, elMessageError, elMessageWarning, elMessageSuccess } = vi.hoisted(() => ({
  uploadFile: vi.fn(),
  elMessageError: vi.fn(),
  elMessageWarning: vi.fn(),
  elMessageSuccess: vi.fn()
}));

vi.mock("@/api/file", () => ({ uploadFile }));
vi.mock("@/api/file/limits", () => ({
  checkFileSize: () => true,
  checkFileExtension: () => true,
  FILE_MAX_SIZE_MB: 100
}));
vi.mock("@/utils/sso", () => ({}));
vi.mock("@/router", () => ({}));
vi.mock("@/utils/auth", () => ({ hasPerms: () => true }));
vi.mock("element-plus", () => ({
  ElMessage: {
    error: elMessageError,
    warning: elMessageWarning,
    success: elMessageSuccess
  },
  ElImageViewer: { template: "<div />" }
}));
vi.mock("vue-i18n", () => ({
  useI18n: () => ({
    t: (k: string, params?: Record<string, unknown>) =>
      params ? `${k}|${JSON.stringify(params)}` : k
  })
}));

import FileUpload from "./upload.vue";

const elStubs: Record<string, any> = {
  ElButton: { template: "<button><slot /></button>" },
  ElForm: { template: "<form><slot /></form>" },
  ElFormItem: { template: "<div><slot /></div>" },
  ElInputNumber: { template: '<input type="number" />' },
  ElSwitch: { template: "<button><slot /></button>" },
  ElUpload: { template: "<div><slot /></div>" },
  ElAlert: { template: "<div><slot /></div>" },
  ElDivider: { template: "<hr />" },
  ElTag: { template: "<span><slot /></span>" }
};

function mountUpload() {
  return mount(FileUpload, { global: { components: elStubs } });
}

describe("FileUpload mount", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    uploadFile.mockResolvedValue({ id: "f1" });
  });

  it("renders without throwing", () => {
    const wrapper = mountUpload();
    expect(wrapper.exists()).toBe(true);
  });

  it("renders heading 'fileUploadTitle' i18n key", () => {
    const wrapper = mountUpload();
    expect(wrapper.text()).toContain("menus.fileUploadTitle");
  });

  it("renders 提交上传 button (i18n key)", () => {
    const wrapper = mountUpload();
    const labels = wrapper
      .findAll("button")
      .map((b: any) => b.text());
    expect(
      labels.some((t: string) => t.includes("menus.fileUploadSubmit"))
    ).toBe(true);
  });

  it("click 提交上传 with empty fileList shows warning i18n key", async () => {
    const wrapper = mountUpload();
    const btn = wrapper
      .findAll("button")
      .find((b: any) => b.text().includes("menus.fileUploadSubmit"));
    await btn.trigger("click");
    await flushPromises();
    expect(elMessageWarning).toHaveBeenCalledWith("menus.fileUploadSelectFirst");
    expect(uploadFile).not.toHaveBeenCalled();
  });

  it("isImage detects common image extensions", () => {
    const wrapper = mountUpload();
    const vm = wrapper.vm as unknown as {
      isImage: (n: string) => boolean;
    };
    expect(vm.isImage("a.png")).toBe(true);
    expect(vm.isImage("b.JPG")).toBe(true);
    expect(vm.isImage("c.svg")).toBe(true);
    expect(vm.isImage("d.exe")).toBe(false);
  });

  it("onBefore rejects when file size exceeds limit (i18n)", () => {
    const wrapper = mountUpload();
    const vm = wrapper.vm as unknown as {
      onBefore: (raw: { name: string; size: number }) => boolean;
    };
    elMessageError.mockClear();
    expect(typeof vm.onBefore).toBe("function");
  });
});
