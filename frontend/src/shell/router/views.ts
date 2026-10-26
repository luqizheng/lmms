import type { RouteComponent } from "vue-router";

/**
 * 视图解析器：把 DB 菜单的 component 字符串映射到实际视图文件。
 *
 * 只解析**新壳视图**（src/shell/views）。legacy pure-admin 视图不再可达 ——
 * 它们属于旧壳（element-plus + 旧布局），在 PRISM 壳里无法正确渲染。
 * 未迁移的菜单项会落到「该页面正在迁移」占位，而不是加载一个坏掉的页面。
 */
const shellViews = import.meta.glob("../views/**/*.vue");

function normalize(component: string): string {
  return component
    .replace(/^\/+/, "")
    .replace(/\.vue$/, "")
    .replace(/^views\//, "")
    .replace(/^index$/, "index");
}

/** 候选 key */
function candidates(component: string): string[] {
  const path = normalize(component);
  return [`../views/${path}.vue`, `../views/${path}/index.vue`];
}

/**
 * 解析组件。找不到时返回 undefined —— 调用方给出「页面建设中」占位，
 * 而不是伪造一个空页面。
 */
export function resolveView(component?: string): RouteComponent | undefined {
  if (!component) return undefined;
  for (const key of candidates(component)) {
    const loader = shellViews[key];
    if (loader) return loader as RouteComponent;
  }
  return undefined;
}

export function hasView(component?: string): boolean {
  return resolveView(component) !== undefined;
}
