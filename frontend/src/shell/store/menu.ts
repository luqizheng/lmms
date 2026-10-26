import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { getAsyncRoutes, type RouteNode } from "../api";

/** 壳使用的菜单节点 */
export interface MenuNode {
  id: string;
  name: string;
  path: string;
  icon?: string;
  component?: string;
  children: MenuNode[];
}

/**
 * 把后端返回的动态路由树转成壳的菜单树。
 *
 * 协议来源：prue_backend `GET /v1/pure/get-async-routes` —— 后端只返回**已激活**
 * 的那个菜单配置版本，菜单可见性由 `meta.showLink` 决定。
 *
 * 规则：
 *   - `meta.showLink === false` 的节点不进入菜单（路由仍会注册，可直接访问）
 *   - 既没有路径也没有子节点的节点不展示
 *   - 子节点递归处理；父节点被隐藏时，其子树整体不展示（后端已按父子结构下发，
 *     这里不做"提升"——隐藏的分组不应该露出孤儿菜单）
 */
export function normalizeMenus(items: RouteNode[]): MenuNode[] {
  const result: MenuNode[] = [];

  for (const item of items ?? []) {
    if (!item) continue;
    if (item.meta?.showLink === false) continue;

    const children = normalizeMenus(item.children ?? []);
    const node: MenuNode = {
      id: item.name ?? item.path,
      name: item.meta?.title ?? item.name ?? item.path,
      path: item.path ?? "",
      icon: item.meta?.icon,
      component: item.component,
      children
    };

    if (!node.path && children.length === 0) continue;
    result.push(node);
  }

  return result;
}

/** 展平菜单树，得到 path → 节点 的索引 */
export function flattenMenus(nodes: MenuNode[]): Map<string, MenuNode> {
  const map = new Map<string, MenuNode>();
  const walk = (list: MenuNode[]) => {
    for (const node of list) {
      if (node.path) map.set(node.path, node);
      if (node.children.length) walk(node.children);
    }
  };
  walk(nodes);
  return map;
}

export const useMenuStore = defineStore("prism-menu", () => {
  const menus = ref<MenuNode[]>([]);
  const loading = ref(false);
  const loaded = ref(false);

  const index = computed(() => flattenMenus(menus.value));

  async function load(): Promise<MenuNode[]> {
    loading.value = true;
    try {
      const routes = await getAsyncRoutes();
      menus.value = normalizeMenus(routes);
      loaded.value = true;
      return menus.value;
    } finally {
      loading.value = false;
    }
  }

  function reset() {
    menus.value = [];
    loaded.value = false;
  }

  function findByPath(path: string): MenuNode | undefined {
    return index.value.get(path);
  }

  return { menus, loading, loaded, load, reset, findByPath };
});
