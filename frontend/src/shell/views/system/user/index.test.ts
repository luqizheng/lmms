// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import UserView from "./index.vue";

const get = vi.fn();
const post = vi.fn();
const put = vi.fn();

vi.mock("@/shell/api", () => ({
  http: {
    get: (...args: unknown[]) => get(...args),
    post: (...args: unknown[]) => post(...args),
    put: (...args: unknown[]) => put(...args)
  }
}));

vi.mock("@/shell/store/auth", () => ({
  useAuthStore: () => ({
    hasPermission: () => true
  })
}));

describe("系统 / 用户管理", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    get.mockResolvedValue({ items: [], total: 0 });
  });

  it("加载后渲染接口返回的用户", async () => {
    get.mockResolvedValueOnce({
      items: [
        {
          id: "1",
          username: "admin",
          nickname: "管理员",
          email: "a@b.c",
          status: "active",
          created_at: "2026-01-01"
        }
      ],
      total: 1
    });
    const w = mountShell(UserView);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    expect(get).toHaveBeenCalledWith(
      "/v1/iam/users",
      expect.objectContaining({ page: 1, size: 10 })
    );
    expect(w.text()).toContain("admin");
    expect(w.text()).toContain("管理员");
  });

  it("状态列用 Badge 呈现", async () => {
    get.mockResolvedValueOnce({
      items: [
        {
          id: "1",
          username: "u",
          nickname: "n",
          status: "disabled",
          created_at: ""
        }
      ],
      total: 1
    });
    const w = mountShell(UserView);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    expect(w.find(".ui-badge").exists()).toBe(true);
  });

  it("新建时提交 POST，创建成功后重新加载列表", async () => {
    get.mockResolvedValue({ items: [], total: 0 });
    post.mockResolvedValueOnce({});
    const w = mountShell(UserView);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    const create = w.findAll("button").find(b => b.text() === "新建用户");
    await create?.trigger("click");
    await nextTick();

    const form = document.querySelector("form");
    form?.dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true })
    );
    await nextTick();
    await nextTick();

    expect(post).toHaveBeenCalledWith("/v1/iam/users", expect.anything());
    expect(get).toHaveBeenCalledTimes(2); // 首次加载 + 提交后刷新
  });
});
