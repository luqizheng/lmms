// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mountShell } from "../test-utils";
import Sidebar from "./Sidebar.vue";
import { resolveIcon } from "./icons";

const menus = [
  {
    id: "system",
    name: "系统管理",
    path: "/system",
    icon: "settings",
    children: [
      {
        id: "user",
        name: "用户管理",
        path: "/system/user",
        icon: "users",
        children: [
          {
            id: "user-detail",
            name: "用户详情",
            path: "/system/user/:id",
            children: []
          }
        ]
      }
    ]
  }
];

describe("Sidebar", () => {
  it("渲染菜单项与层级", () => {
    const w = mountShell(Sidebar, {
      props: { menus, activePath: "/system/user" }
    });
    expect(w.text()).toContain("系统管理");
    expect(w.text()).toContain("用户管理");
  });

  it("当前项标记 aria-current=page", () => {
    const w = mountShell(Sidebar, {
      props: { menus, activePath: "/system/user" }
    });
    const current = w.findAll("[aria-current='page']");
    expect(current.length).toBe(1);
  });

  it("图标按 Lucide 名称渲染成组件，而不是把名字当文本", () => {
    const w = mountShell(Sidebar, { props: { menus, activePath: "/system" } });
    expect(w.find(".ui-sidebar__icon").exists()).toBe(true);
    expect(w.text()).not.toContain("settings");
  });

  it("未知图标名不渲染图标", () => {
    expect(resolveIcon("not-a-real-icon")).toBeUndefined();
    expect(resolveIcon(undefined)).toBeUndefined();
    expect(resolveIcon("users")).toBeTruthy();
  });
});
