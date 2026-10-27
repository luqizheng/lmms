// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createRouter, createWebHashHistory, type Router } from "vue-router";
import { createPinia, setActivePinia } from "pinia";

const getAsyncRoutes = vi.fn();
const authenticated = { value: true };

vi.mock("../api", async () => {
  const actual = await vi.importActual<typeof import("../api")>("../api");
  return {
    ...actual,
    getAsyncRoutes: (...args: unknown[]) => getAsyncRoutes(...args)
  };
});

vi.mock("../store/auth", () => ({
  useAuthStore: () => ({ authenticated: authenticated.value })
}));

const menuState = { loaded: false };

vi.mock("../store/menu", async () => {
  const actual =
    await vi.importActual<typeof import("../store/menu")>("../store/menu");
  return {
    ...actual,
    useMenuStore: () => ({
      menus: [],
      loading: false,
      get loaded() {
        return menuState.loaded;
      },
      load: async () => {
        const routes = await getAsyncRoutes();
        const nodes = actual.normalizeMenus(routes);
        menuState.loaded = true;
        return nodes;
      },
      reset: () => {
        menuState.loaded = false;
      },
      findByPath: () => undefined
    })
  };
});

const { installAuthGuard, buildRoutes } = await import("./guard");

function makeRouter(): Router {
  const router = createRouter({
    history: createWebHashHistory(),
    routes: [
      {
        path: "/login",
        name: "login",
        component: { template: "<div />" },
        meta: { public: true }
      },
      {
        path: "/",
        name: "shell",
        component: { template: "<div />" },
        children: [
          {
            path: "dashboard",
            name: "dashboard",
            component: { template: "<div />" }
          }
        ]
      },
      {
        path: "/:pathMatch(.*)*",
        name: "not-found",
        component: { template: "<div />" }
      }
    ]
  });
  return router;
}

describe("installAuthGuard", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    getAsyncRoutes.mockReset();
    authenticated.value = true;
    menuState.loaded = false;
  });

  it("公开路由直接放行", async () => {
    const router = makeRouter();
    installAuthGuard(router);
    await router.push("/login");
    expect(router.currentRoute.value.name).toBe("login");
  });

  it("未登录跳登录页并带 redirect", async () => {
    authenticated.value = false;
    const router = makeRouter();
    installAuthGuard(router);
    await router.push("/dashboard");
    expect(router.currentRoute.value.name).toBe("login");
    expect(router.currentRoute.value.query.redirect).toBe("/dashboard");
  });

  it("装载菜单并注册动态路由", async () => {
    getAsyncRoutes.mockResolvedValue([
      {
        path: "/system/user",
        name: "user",
        component: "system/user",
        meta: { title: "用户管理" }
      }
    ]);
    const router = makeRouter();
    installAuthGuard(router);
    await router.push("/dashboard");
    expect(getAsyncRoutes).toHaveBeenCalled();
    expect(router.hasRoute("menu-user")).toBe(true);
  });

  it("注册动态路由后返回 replace 以重新解析当前目标（深链不落 404）", async () => {
    getAsyncRoutes.mockResolvedValue([
      {
        path: "/system/user",
        name: "user",
        component: "system/user",
        meta: { title: "用户管理" }
      }
    ]);
    const router = makeRouter();

    // 捕获注册的守卫，直接以目标路由调用它
    let guard: ((to: unknown) => unknown) | undefined;
    const original = router.beforeEach.bind(router);
    router.beforeEach = ((fn: never) => {
      guard = fn as unknown as (to: unknown) => unknown;
      return original(fn);
    }) as typeof router.beforeEach;

    installAuthGuard(router);

    const to = { path: "/system/user", meta: {}, fullPath: "/system/user" };
    const outcome = await guard?.(to);

    expect(outcome).toBeTruthy();
    expect((outcome as { replace?: boolean }).replace).toBe(true);
    expect(router.hasRoute("menu-user")).toBe(true);
  });

  it("整页刷新深链命中 404 兜底时，仍加载菜单并重解析到动态路由", async () => {
    getAsyncRoutes.mockResolvedValue([
      {
        path: "/pm/projects/list",
        name: "projects-list",
        component: "pm/projects/list",
        meta: { title: "项目列表" }
      }
    ]);
    const router = makeRouter();
    installAuthGuard(router);

    // 动态路由尚未注册，深链先落到 catch-all（not-found）
    await router.push("/pm/projects/list");

    expect(router.hasRoute("menu-projects-list")).toBe(true);
    expect(router.currentRoute.value.path).toBe("/pm/projects/list");
    expect(router.currentRoute.value.name).toBe("menu-projects-list");
  });

  it("菜单拉取失败时不阻断路由", async () => {
    getAsyncRoutes.mockRejectedValue(new Error("boom"));
    const router = makeRouter();
    installAuthGuard(router);
    await router.push("/dashboard");
    expect(router.currentRoute.value.name).toBe("dashboard");
  });
});

describe("buildRoutes", () => {
  it("为菜单节点生成路由记录", () => {
    const routes = buildRoutes([
      {
        id: "user",
        name: "用户管理",
        path: "/system/user",
        component: "system/user",
        children: []
      }
    ]);
    expect(routes[0].path).toBe("system/user");
    expect(routes[0].name).toBe("menu-user");
    expect(routes[0].meta?.title).toBe("用户管理");
  });

  it("解析不到的组件回落到占位页", () => {
    const routes = buildRoutes([
      {
        id: "x",
        name: "x",
        path: "/x",
        component: "does/not/exist",
        children: []
      }
    ]);
    expect(typeof routes[0].component).toBe("function");
  });
});
