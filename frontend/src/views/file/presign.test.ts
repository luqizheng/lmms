// @vitest-environment happy-dom
import { describe, expect, it, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

vi.hoisted(() => {
  process.env.VITE_ROUTER_HISTORY = "hash";
});

const { createPresign, message } = vi.hoisted(() => ({
  createPresign: vi.fn(),
  message: vi.fn()
}));

vi.mock("@/api/file", () => ({ createPresign }));
vi.mock("@/utils/message", () => ({ message }));
vi.mock("@/utils/sso", () => ({}));
vi.mock("@/router", () => ({}));
vi.mock("@/utils/auth", () => ({ hasPerms: () => true }));
vi.mock("vue-i18n", () => ({
  useI18n: () => ({
    t: (k: string, params?: Record<string, unknown>) =>
      params ? `${k}|${JSON.stringify(params)}` : k
  })
}));
vi.mock("@/utils/error-translate", () => ({
  translateError: (code: unknown) => `error:${code}`
}));

import FilePresign from "./presign.vue";

const elStubs: Record<string, any> = {
  ElButton: { template: "<button><slot /></button>" },
  ElForm: { template: "<form><slot /></form>" },
  ElFormItem: { template: "<div><slot /></div>" },
  ElInput: { template: "<input />" },
  ElInputNumber: { template: '<input type="number" />' },
  ElRadioGroup: { template: "<div><slot /></div>" },
  ElRadio: { template: "<label><slot /></label>" },
  ElCard: { template: "<div><slot /></div>" },
  ElAlert: { template: "<div><slot /></div>" }
};

function mountPresign() {
  return mount(FilePresign, { global: { components: elStubs } });
}

describe("FilePresign mount", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    createPresign.mockResolvedValue({
      url: "https://example.com/presigned",
      expires_at: "2026-12-31T00:00:00Z"
    });
  });

  it("renders without throwing", () => {
    const wrapper = mountPresign();
    expect(wrapper.exists()).toBe(true);
  });

  it("renders heading via i18n key", () => {
    const wrapper = mountPresign();
    expect(wrapper.text()).toContain("menus.presignTitle");
  });

  it("renders 生成 URL button via i18n key", () => {
    const wrapper = mountPresign();
    const labels = wrapper
      .findAll("button")
      .map((b: any) => b.text());
    expect(labels.some((t: string) => t.includes("menus.presignGenerateBtn"))).toBe(true);
  });

  it("click 生成 URL with empty file_id shows message warning", async () => {
    const wrapper = mountPresign();
    const btn = wrapper
      .findAll("button")
      .find((b: any) => b.text().includes("menus.presignGenerateBtn"));
    await btn.trigger("click");
    await flushPromises();
    expect(message).toHaveBeenCalledWith(
      "menus.presignInputFileIdFirst",
      expect.objectContaining({ type: "warning" })
    );
    expect(createPresign).not.toHaveBeenCalled();
  });
});
