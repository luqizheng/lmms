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

vi.mock("@/api/file", () => ({
  listFiles,
  deleteFile,
  // 与 src/api/file/file-url.ts 同步改,两边保持一致
  fileUrl: (path?: string | null): string => {
    if (!path) return "";
    const trimmed = path.trim();
    if (!trimmed) return "";
    if (/^https?:\/\//i.test(trimmed)) return trimmed;
    const base =
      (import.meta.env.VITE_API_BASE as string | undefined) ?? "/api";
    const prefix = base.endsWith("/") ? base.slice(0, -1) : base;
    const suffix = trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
    return `${prefix}${suffix}`;
  }
}));
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
vi.mock("@/utils/error-translate", () => ({
  translateError: (code: unknown) => `error:${code}`
}));

import FileList from "./index.vue";

const elStubs: Record<string, any> = {
  ElButton: { template: "<button><slot /></button>" },
  ElForm: { template: "<form><slot /></form>" },
  ElFormItem: { template: "<div><slot /></div>" },
  ElInput: { template: "<input />" },
  ElTag: { template: '<span class="el-tag"><slot /></span>' },
  ElImage: { template: '<img class="el-image-stub" />' }
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

  it("renders title via PureTableBar i18n key", () => {
    const wrapper = mountFile();
    const bar = wrapper.find(".pure-table-bar-stub");
    expect(bar.attributes("title")).toBe("menus.fileColumnFilename");
  });

  it("renders buttons via i18n keys", () => {
    const wrapper = mountFile();
    const labels = wrapper.findAll("button").map((b: any) => b.text());
    expect(
      labels.some((t: string) => t.includes("menus.notificationSearch"))
    ).toBe(true);
    expect(labels.some((t: string) => t.includes("menus.tenantRefresh"))).toBe(
      true
    );
    expect(labels.some((t: string) => t.includes("menus.fileNew"))).toBe(true);
  });

  it("on 搜索 click calls listFiles", async () => {
    const wrapper = mountFile();
    const searchBtn = wrapper
      .findAll("button")
      .find((b: any) => b.text().includes("menus.notificationSearch"));
    await searchBtn.trigger("click");
    await flushPromises();
    expect(listFiles).toHaveBeenCalled();
  });

  it("on 上传 click pushes /file/upload route", async () => {
    const wrapper = mountFile();
    const uploadBtn = wrapper
      .findAll("button")
      .find((b: any) => b.text().includes("menus.fileNew"));
    await uploadBtn.trigger("click");
    expect(routerPush).toHaveBeenCalledWith("/file/upload");
  });

  it("hides 批量删除 button when no rows selected", () => {
    const wrapper = mountFile();
    const labels = wrapper.findAll("button").map((b: any) => b.text());
    expect(labels.some((t: string) => t.includes("menus.fileBulkDelete"))).toBe(
      false
    );
  });

  it("renders drop zone overlay (hidden by default)", () => {
    const wrapper = mountFile();
    expect(wrapper.find(".drop-zone-wrap").exists()).toBe(true);
    expect(wrapper.find(".is-drag-over").exists()).toBe(false);
  });
});
