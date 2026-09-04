import { describe, it, expect } from "vitest";
import {
  buildStaticMenuSeed,
  mergeMenus,
  isStaticMenuPath,
  type OverlayRouteNode
} from "./menu-merge";

const Layout = () => import("@/layout/index.vue");

const staticRoutes: OverlayRouteNode[] = [
  {
    path: "/",
    name: "Home",
    component: Layout,
    meta: { title: "首页", rank: 0 },
    children: [
      {
        path: "/welcome",
        name: "Welcome",
        component: () => import("@/views/welcome.vue"),
        meta: { title: "首页", showLink: true }
      }
    ]
  },
  {
    path: "/system",
    name: "System",
    component: Layout,
    meta: { title: "系统管理", icon: "ep/set-up", rank: 14 },
    children: [
      {
        path: "/system/user",
        name: "SystemUser",
        component: () => import("@/views/system/user/index.vue"),
        meta: { title: "用户管理", roles: ["platform_admin"] }
      },
      {
        path: "/system/menu",
        name: "SystemMenu",
        component: () => import("@/views/system/menu/index.vue"),
        meta: { title: "菜单管理", roles: ["platform_admin"] }
      }
    ]
  },
  {
    path: "/error",
    name: "Error",
    component: Layout,
    meta: { title: "异常页", rank: 10 },
    children: [
      {
        path: "/error/403",
        name: "403",
        component: () => import("@/views/error/403.vue"),
        meta: { title: "403" }
      }
    ]
  },
  {
    path: "/guide",
    name: "Guide",
    component: Layout,
    meta: { title: "引导", rank: 26 },
    children: [
      {
        path: "/guide/index",
        name: "GuidePage",
        component: () => import("@/views/guide/index.vue"),
        meta: { title: "引导" }
      }
    ]
  }
];

describe("buildStaticMenuSeed", () => {
  it("不输出组件函数，且 error/guide 默认 showLink=false", () => {
    const seed = buildStaticMenuSeed(staticRoutes);
    const json = JSON.stringify(seed);
    expect(json).not.toContain("component");

    const error403 = seed
      .find(n => n.path === "/error")!
      .children!.find(c => c.path === "/error/403")!;
    expect(error403.meta?.showLink).toBe(false);

    const guide = seed.find(n => n.path === "/guide")!;
    expect(guide.meta?.showLink).toBe(false);
    expect(guide.children![0].meta?.showLink).toBe(false);
  });

  it("非 demo 静态菜单保留原 showLink/meta", () => {
    const seed = buildStaticMenuSeed(staticRoutes);
    const welcome = seed
      .find(n => n.path === "/")!
      .children!.find(c => c.path === "/welcome")!;
    expect(welcome.meta?.showLink).toBe(true);
    expect(welcome.meta?.title).toBe("首页");
  });

  it("同一 path 在树中只保留首个（去重，规避后端 duplicate route.path）", () => {
    const duplicated: OverlayRouteNode[] = [
      ...staticRoutes,
      {
        path: "/system",
        name: "SystemDuplicate",
        meta: { title: "重复系统" },
        children: [{ path: "/system/user", name: "DupUser" }]
      },
      { path: "/login", name: "Login", meta: { showLink: false } }
    ];
    const seed = buildStaticMenuSeed(duplicated);

    const allPaths: string[] = [];
    const walk = (nodes: OverlayRouteNode[]) =>
      nodes.forEach(n => {
        allPaths.push(n.path);
        if (n.children) walk(n.children);
      });
    walk(seed);

    expect(allPaths.filter(p => p === "/system")).toHaveLength(1);
    expect(allPaths.filter(p => p === "/system/user")).toHaveLength(1);
    expect(seed.find(n => n.path === "/system")?.meta?.title).toBe("系统管理");
  });
});

describe("mergeMenus", () => {
  it("DB overlay 覆盖静态 meta 但保留静态 component", () => {
    const db: OverlayRouteNode[] = [
      {
        path: "/system",
        meta: { title: "系统设置(自定义)", icon: "ep/star" },
        children: [
          { path: "/system/user", meta: { showLink: false } }
        ]
      }
    ];
    const merged = mergeMenus(db, staticRoutes);
    const system = merged.find(n => n.path === "/system")!;
    expect(system._static).toBe(true);
    expect(typeof system.component).toBe("function"); // 真实组件保留
    expect(system.meta?.title).toBe("系统设置(自定义)"); // DB 覆盖
    expect(system.meta?.icon).toBe("ep/star");
    expect(system.meta?.rank).toBe(14); // 静态 meta 保留

    const user = system.children!.find(c => c.path === "/system/user")!;
    expect(user.meta?.showLink).toBe(false);
    expect(typeof user.component).toBe("function");
  });

  it("DB 未覆盖的静态节点被保留（静态兜底）", () => {
    const merged = mergeMenus([], staticRoutes);
    expect(merged.some(n => n.path === "/system")).toBe(true);
    const system = merged.find(n => n.path === "/system")!;
    expect(system.children!.some(c => c.path === "/system/menu")).toBe(true);
  });

  it("DB 自定义外链节点（静态无同 path）追加到顶层", () => {
    const db: OverlayRouteNode[] = [
      {
        path: "https://example.com",
        name: "External",
        component: "external",
        meta: { title: "外链", roles: ["platform_admin"] }
      }
    ];
    const merged = mergeMenus(db, staticRoutes);
    const ext = merged.find(n => n.path === "https://example.com")!;
    expect(ext).toBeTruthy();
    expect(ext._static).toBeUndefined();
    expect(ext.component).toBe("external"); // 字符串组件路径保留，供 addAsyncRoutes 解析
  });

  it("DB meta.roles=[] 覆盖静态 roles=[platform_admin]（空数组=所有人可见）", () => {
    const db: OverlayRouteNode[] = [
      { path: "/system/user", meta: { roles: [] } }
    ];
    const merged = mergeMenus(db, staticRoutes);
    const system = merged.find(n => n.path === "/system")!;
    const user = system.children!.find(c => c.path === "/system/user")!;
    expect(user.meta?.roles).toEqual([]);
  });
});

describe("isStaticMenuPath", () => {
  it("命中静态 path 返回 true，自定义 path 返回 false", () => {
    expect(isStaticMenuPath("/system/user", staticRoutes)).toBe(true);
    expect(isStaticMenuPath("/system/unknown", staticRoutes)).toBe(false);
    expect(isStaticMenuPath("https://example.com", staticRoutes)).toBe(false);
  });
});
