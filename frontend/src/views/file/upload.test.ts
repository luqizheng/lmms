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
  }
}));

import FileUpload from "./upload.vue";

const elStubs: Record<string, any> = {
  ElButton: { template: "<button><slot /></button>" },
  ElForm: { template: "<form><slot /></form>" },
  ElFormItem: { template: "<div><slot /></div>" },
  ElInputNumber: { template: '<input type="number" />' },
  ElSwitch: { template: "<button><slot /></button>" },
  ElUpload: { template: "<div><slot /></div>" },
  ElAlert: { template: "<div><slot /></div>" }
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

  it("renders heading '上传文件'", () => {
    const wrapper = mountUpload();
    expect(wrapper.text()).toContain("上传文件");
  });

  it("renders 提交上传 button", () => {
    const wrapper = mountUpload();
    const labels = wrapper
      .findAll("button")
      .map((b: any) => b.text());
    expect(labels.some((t: string) => t.includes("提交上传"))).toBe(true);
  });

  it("click 提交上传 with empty fileList shows warning", async () => {
    const wrapper = mountUpload();
    const btn = wrapper
      .findAll("button")
      .find((b: any) => b.text().includes("提交上传"));
    await btn.trigger("click");
    await flushPromises();
    expect(elMessageWarning).toHaveBeenCalledWith("请先选文件");
    expect(uploadFile).not.toHaveBeenCalled();
  });
});
