import { cloneDeep } from "@pureadmin/utils";

/**
 * 菜单合并模型：DB 为主、静态兜底。
 *
 * - 静态菜单（src/router/modules/*）提供真实组件（component 函数 / Layout）与默认 meta，
 *   始终注册在路由表里，页面永远可直接访问。
 * - DB（prue_backend 菜单配置）只存“可序列化的覆盖项”：meta（showLink / roles / title /
 *   icon / rank ...）以及纯自定义的外链 / iframe / 按钮节点。
 * - 合并时按 path 匹配：DB 有的条目用 DB 的 meta 覆盖静态 meta（roles 为空数组表示
 *   “所有人可见”，会覆盖静态的 roles 限制）；DB 没有的静态条目回退代码默认值；
 *   DB 里有、静态里没有的自定义节点保留在各自父级下（顶层自定义节点追加到结果顶层）。
 */

export interface OverlayRouteNode {
  path: string;
  name?: string;
  redirect?: string;
  /** 静态节点为真实组件（函数/对象，不可序列化）；DB 自定义节点为字符串路径 */
  component?: unknown;
  meta?: Record<string, any>;
  children?: OverlayRouteNode[];
  /** 内部标记：该节点来自静态代码（组件已注册，addAsyncRoutes 跳过组件解析） */
  _static?: boolean;
  [key: string]: unknown;
}

/** pure-admin demo 页：异常页与引导页，种子里默认不在侧边栏显示 */
function isDemoPath(path: string): boolean {
  return (
    path === "/guide" ||
    path.startsWith("/guide/") ||
    path.startsWith("/error")
  );
}

/** path 是否命中任一静态菜单节点 */
export function isStaticMenuPath(
  path: string,
  staticRoutes: OverlayRouteNode[]
): boolean {
  for (const node of staticRoutes ?? []) {
    if (!node) continue;
    if (node.path === path) return true;
    if (node.children && isStaticMenuPath(path, node.children)) return true;
  }
  return false;
}

/**
 * 从静态菜单树生成可存入 DB 的种子（纯数据，无组件函数）。
 * /error/** 与 /guide/** 默认 showLink=false，其余保留静态 meta。
 */
export function buildStaticMenuSeed(
  staticRoutes: OverlayRouteNode[]
): OverlayRouteNode[] {
  const seed = (nodes: OverlayRouteNode[]): OverlayRouteNode[] =>
    (nodes ?? [])
      .filter(node => node && node.path)
      .map(node => {
        const meta: Record<string, any> = cloneDeep(node.meta ?? {});
        if (isDemoPath(node.path)) {
          meta.showLink = false;
        }
        const out: OverlayRouteNode = { path: node.path, meta };
        if (node.name) out.name = node.name;
        if (node.redirect) out.redirect = node.redirect;
        const children = seed(node.children ?? []);
        if (children.length) out.children = children;
        return out;
      });
  return seed(staticRoutes);
}

function indexByPath(
  nodes: OverlayRouteNode[],
  acc: Map<string, OverlayRouteNode> = new Map()
): Map<string, OverlayRouteNode> {
  for (const node of nodes ?? []) {
    if (node?.path) acc.set(node.path, node);
    if (node?.children) indexByPath(node.children, acc);
  }
  return acc;
}

/**
 * DB 为主合并静态菜单。返回给 handleAsyncRoutes 的树。
 * 以静态为骨架：每个静态节点用「全局 DB 索引」里同 path 的 overlay 覆盖 meta（不要求
 * overlay 的父级也在 DB 里，任意层级的 DB 覆盖都生效）；DB 里有、静态骨架没有的自定义
 * 节点（外链/iframe/自定义页面）挂到对应父级、顶层自定义追加到结果顶层。
 * 不修改输入（静态与 DB 均克隆）。
 */
export function mergeMenus(
  dbRoutes: OverlayRouteNode[],
  staticRoutes: OverlayRouteNode[]
): OverlayRouteNode[] {
  const dbByPath = indexByPath(dbRoutes ?? []);
  const customSeen = new Set<string>();
  const custom: OverlayRouteNode[] = [];

  // 已被静态骨架消费的自定义 DB 节点（挂在静态父下的），顶层追加时去重用
  const consumedCustom = new Set<string>();

  const mergeNode = (
    staticNode: OverlayRouteNode,
    staticSiblings: OverlayRouteNode[]
  ): OverlayRouteNode => {
    const node: OverlayRouteNode = cloneDeep(staticNode);
    node._static = true;
    const overlay = dbByPath.get(staticNode.path);

    if (overlay) {
      node.meta = { ...(staticNode.meta ?? {}), ...(overlay.meta ?? {}) };
      if (overlay.redirect) node.redirect = overlay.redirect;
    }

    const staticChildren = staticNode.children ?? [];
    const dbChildren = overlay?.children ?? [];
    const children: OverlayRouteNode[] = [];

    // 静态子节点：递归（各自从全局 DB 索引取覆盖）
    for (const child of staticChildren) {
      children.push(mergeNode(child, staticChildren));
    }
    // DB overlay 里的自定义子节点（静态没有同 path）：挂到该静态父下
    for (const dbChild of dbChildren) {
      if (dbChild?.path && !isStaticMenuPath(dbChild.path, [staticNode])) {
        consumedCustom.add(dbChild.path);
        const cloned = cloneDeep(dbChild);
        delete cloned._static;
        children.push(cloned);
      }
    }

    if (children.length) node.children = children;
    return node;
  };

  const result = (staticRoutes ?? [])
    .filter(n => n && n.path)
    .map(n => mergeNode(n, staticRoutes));

  // 顶层 DB 自定义节点（静态骨架完全没有的，且未作为静态父的子节点被消费）
  const collectTopCustom = (nodes: OverlayRouteNode[]) => {
    for (const node of nodes ?? []) {
      if (!node?.path) continue;
      if (
        !isStaticMenuPath(node.path, staticRoutes) &&
        !consumedCustom.has(node.path) &&
        !customSeen.has(node.path)
      ) {
        customSeen.add(node.path);
        const cloned = cloneDeep(node);
        delete cloned._static;
        custom.push(cloned);
      }
      if (node.children) collectTopCustom(node.children);
    }
  };
  collectTopCustom(dbRoutes ?? []);

  return [...result, ...custom];
}
