import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { listMenus, type IamMenu } from "../api";

/** 壳使用的菜单节点 */
export interface MenuNode {
  id: string;
  name: string;
  path: string;
  icon?: string;
  component?: string;
  children: MenuNode[];
}

const ACTIVE_STATUS = "active";

/**
 * 把 DB 菜单转成壳的菜单树。
 *
 * 规则（沿用现有菜单协议）：
 *   - 只保留 visible 为真、status 为 active 的节点
 *   - 按 sort_order 升序
 *   - 父节点被过滤掉时，其子节点提升为顶层
 */
export function normalizeMenus(items: IamMenu[]): MenuNode[] {
  const visible = items
    .filter(item => item.visible !== false && item.status === ACTIVE_STATUS)
    .sort((a, b) => (a.sort_order ?? 0) - (b.sort_order ?? 0));

  const result: MenuNode[] = [];

  for (const item of visible) {
    const children = normalizeMenus(item.children ?? []);
    const node: MenuNode = {
      id: item.id,
      name: item.name,
      path: item.path,
      icon: item.icon,
      component: item.component,
      children
    };
    // 没有任何子节点、且自身没有路由地址的分组项不展示
    if (!item.path && children.length === 0) continue;
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
      const response = await listMenus();
      menus.value = normalizeMenus(response.items ?? []);
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
