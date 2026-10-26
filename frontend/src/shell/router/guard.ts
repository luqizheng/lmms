import type { Router, RouteRecordRaw } from "vue-router";
import { useAuthStore } from "../store/auth";
import { useMenuStore, type MenuNode } from "../store/menu";
import { resolveView } from "./views";

/** 动态菜单挂载点：与 staticRoutes 中 shell 路由同名 */
const SHELL_ROUTE_NAME = "shell";

/** 把菜单树转成路由记录 */
export function buildRoutes(menus: MenuNode[]): RouteRecordRaw[] {
  const routes: RouteRecordRaw[] = [];

  for (const node of menus) {
    if (!node.path) continue;
    const component = resolveView(node.component);
    const children = node.children.length
      ? buildRoutes(node.children)
      : undefined;

    const record = {
      path: node.path.replace(/^\//, ""),
      name: `menu-${node.id}`,
      meta: {
        title: node.name,
        icon: node.icon,
        permission: node.component
      },
      component: component ?? (() => import("../views/ComingSoon.vue")),
      ...(children ? { children } : {})
    } as RouteRecordRaw;

    routes.push(record);
  }

  return routes;
}

/**
 * 安装鉴权守卫：
 *   1. 公开路由直接放行
 *   2. 未登录 → 跳登录页并带 redirect
 *   3. 已登录但菜单未装载 → 拉 DB 菜单并动态挂载路由
 */
export function installAuthGuard(router: Router) {
  router.beforeEach(async to => {
    if (to.meta.public) return true;

    const auth = useAuthStore();
    if (!auth.authenticated) {
      return { name: "login", query: { redirect: to.fullPath } };
    }

    const menu = useMenuStore();
    if (!menu.loaded) {
      try {
        const menus = await menu.load();
        const parent = router
          .getRoutes()
          .find(route => route.name === SHELL_ROUTE_NAME);
        if (parent?.name) {
          for (const route of buildRoutes(menus)) {
            router.addRoute(parent.name as string, route);
          }
        }
      } catch {
        // 菜单拉取失败时不阻断路由：页面会走 404，登录态仍可恢复
        return true;
      }
    }

    return true;
  });
}
