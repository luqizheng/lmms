// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flattenMenus, normalizeMenus } from "./menu";
import type { RouteNode } from "../api";

function node(partial: Partial<RouteNode> & { path: string }): RouteNode {
  return {
    name: partial.path,
    meta: { title: partial.path },
    ...partial
  } as RouteNode;
}

describe("normalizeMenus", () => {
  it("取 meta.title 作为菜单名", () => {
    const nodes = normalizeMenus([
      node({ path: "/user", meta: { title: "用户管理" } })
    ]);
    expect(nodes[0].name).toBe("用户管理");
  });

  it("meta 缺失时回退到 name / path", () => {
    const nodes = normalizeMenus([{ path: "/user", name: "user" }]);
    expect(nodes[0].name).toBe("user");
  });

  it("showLink=false 的节点不进菜单", () => {
    const nodes = normalizeMenus([
      node({ path: "/hidden", meta: { showLink: false } })
    ]);
    expect(nodes).toHaveLength(0);
  });

  it("保留 children 并递归处理", () => {
    const nodes = normalizeMenus([
      node({ path: "/parent", children: [node({ path: "/parent/child" })] })
    ]);
    expect(nodes[0].children).toHaveLength(1);
    expect(nodes[0].children[0].path).toBe("/parent/child");
  });

  it("没有路径也没有子节点的分组项不展示", () => {
    const nodes = normalizeMenus([node({ path: "" })]);
    expect(nodes).toHaveLength(0);
  });

  it("父节点被隐藏时其子树不展示（不做孤儿提升）", () => {
    const nodes = normalizeMenus([
      node({
        path: "/parent",
        meta: { showLink: false },
        children: [node({ path: "/parent/child" })]
      })
    ]);
    expect(nodes).toHaveLength(0);
  });

  it("保留 component 字段供路由解析", () => {
    const nodes = normalizeMenus([
      node({ path: "/user", component: "system/user" })
    ]);
    expect(nodes[0].component).toBe("system/user");
  });
});

describe("flattenMenus", () => {
  it("把树展平成 path → 节点 的索引", () => {
    const nodes = normalizeMenus([
      node({ path: "/p", children: [node({ path: "/p/c" })] })
    ]);
    const index = flattenMenus(nodes);
    expect(index.get("/p")?.id).toBe("/p");
    expect(index.get("/p/c")?.id).toBe("/p/c");
  });

  it("没有 path 的节点不进索引", () => {
    const index = flattenMenus([
      { id: "x", name: "x", path: "", children: [] }
    ]);
    expect(index.size).toBe(0);
  });
});
