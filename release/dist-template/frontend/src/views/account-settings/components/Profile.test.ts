// @vitest-environment happy-dom
import { describe, expect, it, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

const { getMine, uploadFile, message } = vi.hoisted(() => ({
  getMine: vi.fn(),
  uploadFile: vi.fn(),
  message: vi.fn()
}));

vi.mock("@/api/user", () => ({ getMine }));
vi.mock("@/api/file", () => ({ uploadFile }));
vi.mock("@/utils/message", () => ({ message }));
vi.mock("@/utils/sso", () => ({}));
vi.mock("@/router", () => ({}));
vi.mock("@/utils/auth", () => ({ hasPerms: () => true }));
vi.mock("vue-i18n", () => ({
  useI18n: () => ({
    t: (k: string, params?: Record<string, unknown>) =>
      params ? `${k}|${JSON.stringify(params)}` : k
  }),
  createI18n: () => ({}),
  default: {}
}));
vi.mock("@pureadmin/utils", () => ({
  deviceDetection: () => false
}));
vi.mock("@/components/ReCropperPreview", () => ({
  default: {
    name: "ReCropperPreview",
    template: '<div class="re-cropper"></div>',
    methods: { hidePopover: vi.fn() }
  }
}));
vi.mock("@/components/ReIcon/src/hooks", () => ({
  useRenderIcon: () => undefined
}));

import Profile from "./Profile.vue";

const elStubs: Record<string, any> = {
  ElForm: { template: "<form><slot /></form>" },
  ElFormItem: { template: "<div><slot /></div>" },
  ElInput: { template: "<input />" },
  ElButton: { template: "<button><slot /></button>" },
  ElAutocomplete: { template: "<input />" },
  ElAvatar: { template: '<div class="el-avatar"></div>' },
  ElUpload: { template: "<div><slot /></div>" },
  ElDialog: { template: "<div><slot /></div>" },
  ElTextarea: { template: "<textarea />" }
};

function defaultUser() {
  return {
    code: 0,
    data: {
      avatar: "",
      nickname: "alice",
      email: "alice@example.com",
      phone: "13800000000",
      description: "test"
    }
  };
}

function mountProfile() {
  return mount(Profile, { global: { components: elStubs } });
}

describe("Profile mount", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getMine.mockResolvedValue(defaultUser());
    uploadFile.mockResolvedValue({ id: "f1" });
  });

  it("renders without throwing", () => {
    const wrapper = mountProfile();
    expect(wrapper.exists()).toBe(true);
  });

  it("loads user info on mount via getMine", async () => {
    mountProfile();
    await flushPromises();
    expect(getMine).toHaveBeenCalled();
  });

  it("calls real uploadFile on avatar submit (not mock formUpload)", async () => {
    const wrapper = mountProfile();
    await flushPromises();
    const vm = wrapper.vm as unknown as {
      cropperBlob: Blob | undefined;
      handleSubmitImage: () => Promise<void>;
    };
    vm.cropperBlob = new Blob(["fake-png"], { type: "image/png" });
    await vm.handleSubmitImage();
    expect(uploadFile).toHaveBeenCalledTimes(1);
    expect(message).toHaveBeenCalledWith(
      "menus.profileUpdateAvatarSuccess",
      expect.objectContaining({ type: "success" })
    );
  });

  it("shows profileUpdateAvatarFail when uploadFile returns no id", async () => {
    uploadFile.mockResolvedValue({} as any);
    const wrapper = mountProfile();
    await flushPromises();
    const vm = wrapper.vm as unknown as {
      cropperBlob: Blob | undefined;
      handleSubmitImage: () => Promise<void>;
    };
    vm.cropperBlob = new Blob(["x"], { type: "image/png" });
    await vm.handleSubmitImage();
    expect(message).toHaveBeenCalledWith("menus.profileUpdateAvatarFail");
  });

  it("shows profileUpdateAvatarError on uploadFile throw", async () => {
    uploadFile.mockRejectedValue(new Error("network down"));
    const wrapper = mountProfile();
    await flushPromises();
    const vm = wrapper.vm as unknown as {
      cropperBlob: Blob | undefined;
      handleSubmitImage: () => Promise<void>;
    };
    vm.cropperBlob = new Blob(["x"], { type: "image/png" });
    await vm.handleSubmitImage();
    expect(message).toHaveBeenCalledWith(
      expect.stringContaining("menus.profileUpdateAvatarError"),
      expect.objectContaining({ type: "error" })
    );
  });
});
