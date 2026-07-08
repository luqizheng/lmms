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

  it("renders heading '预签名 URL'", () => {
    const wrapper = mountPresign();
    expect(wrapper.text()).toContain("预签名 URL");
  });

  it("renders 生成 URL button", () => {
    const wrapper = mountPresign();
    const labels = wrapper
      .findAll("button")
      .map((b: any) => b.text());
    expect(labels.some((t: string) => t.includes("生成 URL"))).toBe(true);
  });

  it("click 生成 URL with empty file_id shows message warning", async () => {
    const wrapper = mountPresign();
    const btn = wrapper
      .findAll("button")
      .find((b: any) => b.text().includes("生成 URL"));
    await btn.trigger("click");
    await flushPromises();
    expect(message).toHaveBeenCalledWith(
      "请输入 file_id",
      expect.objectContaining({ type: "warning" })
    );
    expect(createPresign).not.toHaveBeenCalled();
  });
});
