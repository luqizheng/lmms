import { RouterView, type Router, type RouteRecordRaw } from "vue-router";
import { defineComponent, h } from "vue";
import { useAuthStore } from "../store/auth";
import { useMenuStore, type MenuNode } from "../store/menu";
import { resolveView } from "./views";

/** 动态菜单挂载点：与 staticRoutes 中 shell 路由同名 */
const SHELL_ROUTE_NAME = "shell";

/** 菜单分组节点：仅透传渲染子路由（分组节点在菜单数据里不带 component） */
const MenuGroupLayout = defineComponent({
  name: "MenuGroupLayout",
  render: () => h(RouterView)
});

/**
 * 把菜单树转成路由记录。
 *
 * `isTop` 为 true 时节点直接挂在 shell（path "/"）下，须用相对路径；
 * 为 false 时是分组节点的子路由，保留绝对路径 —— vue-router 对以 "/" 开头的
 * 子路由按绝对路径解析，否则会被拼成 `/pm/pm/...`。
 */
export function buildRoutes(
  menus: MenuNode[],
  isTop = true
): RouteRecordRaw[] {
  const routes: RouteRecordRaw[] = [];

  for (const node of menus) {
    if (!node.path) continue;
    const component = resolveView(node.component);
    const children = node.children.length
      ? buildRoutes(node.children, false)
      : undefined;

    const record = {
      path: isTop ? node.path.replace(/^\//, "") : node.path,
      name: `menu-${node.id}`,
      meta: {
        title: node.name,
        icon: node.icon,
        permission: node.component
      },
      component:
        component ??
        (children
          ? MenuGroupLayout
          : () => import("../views/ComingSoon.vue")),
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
  // 只在第一次补齐路由后重解析一次：replace 会再次触发本守卫，
  // 若无条件返回 replace 会造成无限重定向。
  let routesRegistered = false;

  router.beforeEach(async to => {
    if (to.meta.public) return true;

    const auth = useAuthStore();
    if (!auth.authenticated) {
      return { name: "login", query: { redirect: to.fullPath } };
    }

    const menu = useMenuStore();
    if (!menu.loaded && !routesRegistered) {
      try {
        const menus = await menu.load();
        const parent = router
          .getRoutes()
          .find(route => route.name === SHELL_ROUTE_NAME);
        if (parent?.name) {
          for (const route of buildRoutes(menus)) {
            router.addRoute(parent.name as string, route);
          }
          // 重新解析当前目标：新注册的路由此时才能被匹配上。
          // vue-router 在进入 beforeEach 之前就已完成目标解析，
          // 缺了这一步，深链（刷新页面 / 直接输地址）会落到 404。
          routesRegistered = true;
          return { ...to, replace: true };
        }
      } catch {
        // 菜单拉取失败时不阻断路由：页面会走 404，登录态仍可恢复
        return true;
      }
    }

    return true;
  });
}
