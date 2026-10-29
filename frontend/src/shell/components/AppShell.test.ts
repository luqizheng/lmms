// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import AppShell from "./AppShell.vue";
import TabBar from "./TabBar.vue";
import { mountShell } from "../test-utils";

const push = vi.fn();
vi.mock("vue-router", () => ({
  useRoute: () => ({ path: "/dashboard", meta: { title: "概览" }, query: {} }),
  useRouter: () => ({ push, go: vi.fn() }),
  RouterView: { template: "<div class='stub-view' />" }
}));

vi.mock("../store/auth", () => ({
  useAuthStore: () => ({
    user: {
      id: "1",
      username: "admin",
      nickname: "管理员",
      roles: ["admin"],
      permissions: []
    },
    authenticated: true,
    signOut: vi.fn().mockResolvedValue(undefined)
  })
}));

vi.mock("../store/menu", () => ({
  useMenuStore: () => ({
    menus: [
      {
        id: "1",
        name: "系统管理",
        path: "/system",
        icon: "settings",
        children: []
      },
      {
        id: "2",
        name: "用户管理",
        path: "/system/user",
        icon: "users",
        children: []
      }
    ],
    findByPath: () => undefined
  })
}));

describe("AppShell", () => {
  it("渲染侧栏、顶栏与主内容区", () => {
    const w = mountShell(AppShell);
    expect(w.find(".ui-sidebar").exists()).toBe(true);
    expect(w.find(".ui-topbar").exists()).toBe(true);
    expect(w.find("main#main-content").exists()).toBe(true);
  });

  it("侧栏渲染 DB 菜单项", () => {
    const w = mountShell(AppShell);
    expect(w.text()).toContain("系统管理");
    expect(w.text()).toContain("用户管理");
  });

  it("提供跳到主内容的跳转链接", () => {
    const w = mountShell(AppShell);
    const skip = w.find(".skip-link");
    expect(skip.attributes("href")).toBe("#main-content");
  });

  it("当前路由进入标签栏", async () => {
    const w = mountShell(AppShell);
    await nextTick();
    expect(w.findAll(".ui-tabbar__tab").length).toBeGreaterThan(0);
  });

  it("标签栏的关闭全部事件清空标签", async () => {
    const w = mountShell(AppShell);
    await nextTick();
    expect(w.findAll(".ui-tabbar__tab").length).toBeGreaterThan(0);
    w.findComponent(TabBar).vm.$emit("closeAll");
    await nextTick();
    expect(w.findAll(".ui-tabbar__tab")).toHaveLength(0);
  });

  it("关闭其他标签时保留目标标签", async () => {
    const w = mountShell(AppShell);
    await nextTick();
    w.findComponent(TabBar).vm.$emit("closeOthers", "/dashboard");
    await nextTick();
    expect(w.findAll(".ui-tabbar__tab")).toHaveLength(1);
    expect(w.find(".ui-tabbar__tab").text()).toContain("概览");
  });
});
