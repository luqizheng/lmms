import type { FormItemProps } from "./types";

export interface RouteMeta {
  title?: string;
  icon?: string;
  extraIcon?: string;
  rank?: number;
  roles?: string[];
  auths?: string[];
  enterTransition?: string;
  leaveTransition?: string;
  activePath?: string;
  frameSrc?: string;
  frameLoading?: boolean;
  keepAlive?: boolean;
  hiddenTag?: boolean;
  fixedTag?: boolean;
  showLink?: boolean;
  showParent?: boolean;
  [key: string]: unknown;
}

export interface RouteMenuCompat {
  id?: number;
  parentId?: number;
  menuType?: number;
  auths?: string;
  roles?: string[];
}

export interface RouteNode {
  path: string;
  name?: string;
  redirect?: string;
  component?: string;
  meta?: RouteMeta;
  children?: RouteNode[];
  _menu?: RouteMenuCompat;
  [key: string]: unknown;
}

function hashId(input: string): number {
  let hash = 0;
  const text = input || `menu_${Date.now()}`;
  for (let index = 0; index < text.length; index += 1) {
    hash = (hash << 5) - hash + text.charCodeAt(index);
    hash |= 0;
  }
  return Math.abs(hash) + 1;
}

function splitAuths(auths: string): string[] | undefined {
  const values = auths
    .split(",")
    .map(item => item.trim())
    .filter(Boolean);
  return values.length > 0 ? values : undefined;
}

function inferMenuType(route: RouteNode): number {
  if (route._menu?.menuType !== undefined) return route._menu.menuType;
  if (route.meta?.frameSrc) return 1;
  if (/^https?:\/\//.test(route.path)) return 2;
  return 0;
}

function toFormItem(route: RouteNode, parentId: number): FormItemProps[] {
  const meta = route.meta ?? {};
  const id =
    route._menu?.id ?? hashId(`${parentId}:${route.path}:${route.name ?? ""}`);
  const auths = route._menu?.auths ?? meta.auths?.join(",") ?? "";

  const current: FormItemProps = {
    id,
    menuType: inferMenuType(route),
    higherMenuOptions: [],
    parentId: route._menu?.parentId ?? parentId,
    title: meta.title ?? "",
    name: route.name ?? "",
    path: route.path ?? "",
    component: route.component ?? "",
    rank: meta.rank ?? 99,
    redirect: route.redirect ?? "",
    icon: meta.icon ?? "",
    extraIcon: meta.extraIcon ?? "",
    enterTransition: meta.enterTransition ?? "",
    leaveTransition: meta.leaveTransition ?? "",
    activePath: meta.activePath ?? "",
    auths,
    frameSrc: meta.frameSrc ?? "",
    frameLoading: meta.frameLoading ?? true,
    keepAlive: meta.keepAlive ?? false,
    hiddenTag: meta.hiddenTag ?? false,
    fixedTag: meta.fixedTag ?? false,
    showLink: meta.showLink ?? true,
    showParent: meta.showParent ?? false,
    roles: route._menu?.roles ?? meta.roles ?? []
  };

  const children =
    route.children?.flatMap(child => toFormItem(child, id)) ?? [];
  return [current, ...children];
}

export function routesToFormItems(routes: RouteNode[]): FormItemProps[] {
  return routes.flatMap(route => toFormItem(route, 0));
}

function itemToRoute(item: FormItemProps, children: RouteNode[]): RouteNode {
  const meta: RouteMeta = {
    title: item.title,
    icon: item.icon,
    extraIcon: item.extraIcon,
    rank: item.rank,
    enterTransition: item.enterTransition,
    leaveTransition: item.leaveTransition,
    activePath: item.activePath,
    frameSrc: item.frameSrc,
    frameLoading: item.frameLoading,
    keepAlive: item.keepAlive,
    hiddenTag: item.hiddenTag,
    fixedTag: item.fixedTag,
    showLink: item.showLink,
    showParent: item.showParent
  };

  const roles = Array.isArray(item.roles) ? item.roles : [];
  meta.roles = roles;

  const auths = splitAuths(item.auths);
  if (auths) meta.auths = auths;

  if (meta.frameLoading === true) delete meta.frameLoading;
  if (meta.keepAlive === false) delete meta.keepAlive;
  if (meta.hiddenTag === false) delete meta.hiddenTag;
  if (meta.fixedTag === false) delete meta.fixedTag;

  Object.keys(meta).forEach(key => {
    if (meta[key] === "" || meta[key] === undefined) delete meta[key];
  });

  const route: RouteNode = {
    path: item.path,
    name: item.name,
    component: item.component,
    meta,
    _menu: {
      id: item.id,
      parentId: item.parentId,
      menuType: item.menuType,
      auths: item.auths,
      roles
    }
  };

  if (item.redirect) route.redirect = item.redirect;
  if (children.length > 0) route.children = children;

  return route;
}

function buildRoutes(items: FormItemProps[], parentId: number): RouteNode[] {
  return items
    .filter(item => (item.parentId ?? 0) === parentId)
    .sort((left, right) => (left.rank ?? 99) - (right.rank ?? 99))
    .map(item => itemToRoute(item, buildRoutes(items, item.id ?? 0)));
}

export function formItemsToRoutes(items: FormItemProps[]): RouteNode[] {
  return buildRoutes(items, 0);
}
