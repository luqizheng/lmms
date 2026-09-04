import { describe, expect, it, vi } from "vitest";
import systemRoutes from "@/router/modules/system";
import { intersection, cloneDeep } from "@pureadmin/utils";

vi.mock("@/utils/sso", () => ({}));

const isOneOfArray = (a: Array<string>, b: Array<string>) =>
  Array.isArray(a) && Array.isArray(b)
    ? intersection(a, b).length > 0
      ? true
      : false
    : true;

// framework copies — equivalent to router/utils.ts:filterNoPermissionTree + filterTree
const filterTree = (data: any[]) => {
  const t = cloneDeep(data).filter(
    (v: any) => v.meta?.showLink !== false
  );
  t.forEach((v: any) => v.children && (v.children = filterTree(v.children)));
  return t;
};
const filterChildrenTree = (data: any[]) => {
  const t = cloneDeep(data).filter((v: any) => v?.children?.length !== 0);
  t.forEach(
    (v: any) => v.children && (v.children = filterTree(v.children))
  );
  return t;
};
const filterNoPermissionTree = (data: any[], currentRoles: string[]) => {
  const t = cloneDeep(data).filter((v: any) =>
    isOneOfArray(v.meta?.roles, currentRoles)
  );
  t.forEach(
    (v: any) => v.children && (v.children = filterNoPermissionTree(v.children, currentRoles))
  );
  return filterChildrenTree(t);
};

const makeSystemTree = () => [
  {
    path: "/system",
    name: "System",
    meta: { title: "系统管理", icon: "ep/set-up" },
    children: [
      { path: "/system/user", name: "SystemUser", meta: { title: "用户", icon: "ep/user", roles: ["tenant_admin", "platform_admin"] } },
      { path: "/system/role", name: "SystemRole", meta: { title: "角色", icon: "ep/user-filled", roles: ["tenant_admin", "platform_admin"] } },
      { path: "/system/permission", name: "SystemPermission", meta: { title: "权限", icon: "ep/lock", roles: ["tenant_admin", "platform_admin"] } },
      { path: "/system/menu", name: "SystemMenu", meta: { title: "菜单", icon: "ep/menu", roles: ["tenant_admin", "platform_admin"] } },
      { path: "/system/tenant", name: "SystemTenant", meta: { title: "租户", icon: "ep/office-building", roles: ["tenant_admin", "platform_admin"] } },
      { path: "/system/dept", name: "SystemDept", meta: { title: "部门", icon: "ep/office-building", roles: ["tenant_admin", "platform_admin"] } }
    ]
  },
  { path: "/dashboard", name: "Dashboard", meta: { title: "首页", icon: "ep/home-filled" } }
];

const runFilter = (userRoles: string[]) => filterNoPermissionTree(makeSystemTree() as any, userRoles);

describe("system routes — meta.roles coverage on 6 sub routes", () => {
  const getChildMeta = (path: string) => {
    const child = (systemRoutes as any).children.find(
      (c: any) => c.path === path
    );
    if (!child) throw new Error(`child not found: ${path}`);
    return child.meta;
  };

  it.each([
    "/system/user",
    "/system/role",
    "/system/permission",
    "/system/menu",
    "/system/tenant",
    "/system/dept"
  ])("declares roles on %s", path => {
    expect(getChildMeta(path).roles).toEqual([
      "tenant_admin",
      "platform_admin"
    ]);
  });
});

describe("filterNoPermissionTree — system menu gating (framework behavior contract)", () => {
  it.each([["tenant_admin"], ["platform_admin"]])(
    "keeps system parent + 6 sub routes for %s",
    role => {
      const filtered = runFilter([role]) as any[];
      const systemNode = filtered.find(n => n.path === "/system");
      expect(systemNode).toBeDefined();
      expect(systemNode.children).toHaveLength(6);
      const childNames = systemNode.children.map((c: any) => c.name).sort();
      expect(childNames).toEqual([
        "SystemDept",
        "SystemMenu",
        "SystemPermission",
        "SystemRole",
        "SystemTenant",
        "SystemUser"
      ]);
    }
  );

  it("keeps top-level menus that have no meta.roles (e.g. /dashboard)", () => {
    const filtered = runFilter(["platform_admin"]) as any[];
    expect(filtered.find(n => n.path === "/dashboard")).toBeDefined();
  });

  it("filters out the entire system subtree for non-admin roles", () => {
    const filtered = runFilter(["common"]) as any[];
    expect(filtered.find(n => n.path === "/system")).toBeUndefined();
    expect(filtered.find(n => n.path === "/dashboard")).toBeDefined();
  });

  it("filters out the entire system subtree when user has no roles", () => {
    const filtered = runFilter([]) as any[];
    expect(filtered.find(n => n.path === "/system")).toBeUndefined();
    expect(filtered.find(n => n.path === "/dashboard")).toBeDefined();
  });
});

describe("isOneOfArray — behavior contract", () => {
  it("returns true when arrays overlap", () => {
    expect(isOneOfArray(["platform_admin"], ["platform_admin"])).toBe(true);
  });

  it("returns false when arrays do not overlap", () => {
    expect(isOneOfArray(["platform_admin"], ["common"])).toBe(false);
  });

  it("returns true when route has no meta.roles (root cause of pre-fix exposure)", () => {
    expect(isOneOfArray(undefined, ["anything"])).toBe(true);
  });
});
