// @vitest-environment happy-dom
import { describe, expect, it, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

vi.hoisted(() => {
  process.env.VITE_ROUTER_HISTORY = "hash";
});

const { listFiles, deleteFile, message, routerPush } = vi.hoisted(() => ({
  listFiles: vi.fn(),
  deleteFile: vi.fn(),
  message: vi.fn(),
  routerPush: vi.fn()
}));

vi.mock("@/api/file", () => ({ listFiles, deleteFile }));
vi.mock("@/api/file/limits", () => ({
  checkFileSize: () => true,
  FILE_MAX_SIZE_MB: 100
}));
vi.mock("@/utils/message", () => ({ message }));
vi.mock("@/utils/sso", () => ({}));
vi.mock("vue-router", () => ({
  useRouter: () => ({ push: routerPush })
}));
vi.mock("@/utils/auth", () => ({ hasPerms: () => true }));
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

import FileList from "./index.vue";

const elStubs: Record<string, any> = {
  ElButton: { template: "<button><slot /></button>" },
  ElForm: { template: "<form><slot /></form>" },
  ElFormItem: { template: "<div><slot /></div>" },
  ElInput: { template: "<input />" }
};

function mountFile() {
  return mount(FileList, { global: { components: elStubs } });
}

describe("FileList mount", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listFiles.mockResolvedValue({ items: [], total: 0 });
  });

  it("renders without throwing", () => {
    const wrapper = mountFile();
    expect(wrapper.exists()).toBe(true);
  });

  it("renders title '文件列表' via PureTableBar", () => {
    const wrapper = mountFile();
    expect(wrapper.find(".pure-table-bar-stub").exists()).toBe(true);
  });

  it("renders 搜索 / 刷新 / 上传 buttons", () => {
    const wrapper = mountFile();
    const labels = wrapper
      .findAll("button")
      .map((b: any) => b.text());
    expect(labels.some((t: string) => t.includes("搜索"))).toBe(true);
    expect(labels.some((t: string) => t.includes("刷新"))).toBe(true);
    expect(labels.some((t: string) => t.includes("上传"))).toBe(true);
  });

  it("on 搜索 click calls listFiles", async () => {
    const wrapper = mountFile();
    const searchBtn = wrapper
      .findAll("button")
      .find((b: any) => b.text().includes("搜索"));
    await searchBtn.trigger("click");
    await flushPromises();
    expect(listFiles).toHaveBeenCalled();
  });

  it("on 上传 click pushes /file/upload route", async () => {
    const wrapper = mountFile();
    const uploadBtn = wrapper
      .findAll("button")
      .find((b: any) => b.text().includes("上传"));
    await uploadBtn.trigger("click");
    expect(routerPush).toHaveBeenCalledWith("/file/upload");
  });
});
