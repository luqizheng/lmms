import { describe, expect, it } from "vitest";
import {
  formItemsToRoutes,
  routesToFormItems,
  type RouteNode
} from "./menu-adapter";

const routeTree: RouteNode[] = [
  {
    path: "/system",
    name: "System",
    component: "LAYOUT",
    redirect: "/system/menu",
    meta: {
      title: "系统管理",
      icon: "ep/setting",
      rank: 1,
      auths: ["system:view"],
      showLink: true,
      showParent: true,
      roles: []
    },
    _menu: {
      id: 1,
      parentId: 0,
      menuType: 0,
      auths: "system:view",
      roles: []
    },
    children: [
      {
        path: "/system/menu",
        name: "SystemMenu",
        component: "system/menu/index",
        meta: {
          title: "菜单管理",
          icon: "ep/menu",
          rank: 2,
          auths: ["system:menu:list"],
          showLink: true,
          showParent: false,
          roles: []
        },
        _menu: {
          id: 2,
          parentId: 1,
          menuType: 0,
          auths: "system:menu:list",
          roles: []
        }
      }
    ]
  }
];

describe("menu-adapter", () => {
  it("converts routes_json to flat menu items", () => {
    const items = routesToFormItems(routeTree);

    expect(items).toHaveLength(2);
    expect(items[0]).toMatchObject({
      id: 1,
      parentId: 0,
      menuType: 0,
      title: "系统管理",
      name: "System",
      path: "/system",
      component: "LAYOUT",
      rank: 1,
      auths: "system:view"
    });
    expect(items[1]).toMatchObject({
      id: 2,
      parentId: 1,
      menuType: 0,
      title: "菜单管理",
      name: "SystemMenu",
      path: "/system/menu",
      component: "system/menu/index",
      rank: 2,
      auths: "system:menu:list"
    });
  });

  it("converts flat menu items back to routes_json with _menu", () => {
    const items = routesToFormItems(routeTree);
    const routes = formItemsToRoutes(items);

    expect(routes).toEqual(routeTree);
  });

  it("generates id and parentId when _menu is missing", () => {
    const items = routesToFormItems([
      {
        path: "/dashboard",
        name: "Dashboard",
        component: "dashboard/index",
        meta: { title: "首页" },
        children: [
          {
            path: "/dashboard/workbench",
            name: "Workbench",
            component: "dashboard/workbench",
            meta: { title: "工作台" }
          }
        ]
      }
    ]);

    expect(items).toHaveLength(2);
    expect(typeof items[0].id).toBe("number");
    expect(items[0].parentId).toBe(0);
    expect(items[1].parentId).toBe(items[0].id);
  });

  it("converts comma separated auths to meta auths", () => {
    const routes = formItemsToRoutes([
      {
        id: 10,
        menuType: 3,
        higherMenuOptions: [],
        parentId: 0,
        title: "按钮",
        name: "ButtonAuth",
        path: "/button-auth",
        component: "",
        rank: 99,
        redirect: "",
        icon: "",
        extraIcon: "",
        enterTransition: "",
        leaveTransition: "",
        activePath: "",
        auths: "a,b, c ",
        frameSrc: "",
        frameLoading: true,
        keepAlive: false,
        hiddenTag: false,
        fixedTag: false,
        showLink: true,
        showParent: false
      }
    ]);

    expect(routes[0].meta?.auths).toEqual(["a", "b", "c"]);
    expect(routes[0]._menu?.auths).toBe("a,b, c ");
  });
});
