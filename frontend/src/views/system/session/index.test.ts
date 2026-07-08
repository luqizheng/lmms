// @vitest-environment happy-dom
import { describe, expect, it, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

vi.hoisted(() => {
  process.env.VITE_ROUTER_HISTORY = "hash";
});

const { listIamSessions, revokeIamSession, message } = vi.hoisted(() => ({
  listIamSessions: vi.fn(),
  revokeIamSession: vi.fn(),
  message: vi.fn()
}));

vi.mock("@/api/iam", () => ({ listIamSessions, revokeIamSession }));
vi.mock("@/utils/message", () => ({ message }));
vi.mock("@/utils/sso", () => ({}));
vi.mock("@/router", () => ({}));
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

import SystemSession from "./index.vue";

const elStubs: Record<string, any> = {
  ElButton: { template: "<button><slot /></button>" },
  ElForm: { template: "<form><slot /></form>" },
  ElFormItem: { template: "<div><slot /></div>" },
  ElTag: { template: "<span><slot /></span>" }
};

function mountSession() {
  return mount(SystemSession, { global: { components: elStubs } });
}

describe("SystemSession mount", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listIamSessions.mockResolvedValue([]);
  });

  it("renders without throwing", () => {
    const wrapper = mountSession();
    expect(wrapper.exists()).toBe(true);
  });

  it("renders title '在线会话' via PureTableBar", () => {
    const wrapper = mountSession();
    const bar = wrapper.find(".pure-table-bar-stub");
    expect(bar.exists()).toBe(true);
  });

  it("renders 刷新 button", () => {
    const wrapper = mountSession();
    const labels = wrapper
      .findAll("button")
      .map((b: any) => b.text());
    expect(labels.some((t: string) => t.includes("刷新"))).toBe(true);
  });

  it("on mount calls listIamSessions", async () => {
    mountSession();
    await flushPromises();
    expect(listIamSessions).toHaveBeenCalled();
  });
});
