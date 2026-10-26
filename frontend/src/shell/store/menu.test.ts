// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flattenMenus, normalizeMenus, type MenuNode } from "./menu";
import type { IamMenu } from "../api";

function menu(partial: Partial<IamMenu> & { id: string }): IamMenu {
  return {
    name: partial.id,
    code: partial.id,
    path: `/${partial.id}`,
    sort_order: 0,
    visible: true,
    status: "active",
    children: [],
    ...partial
  } as IamMenu;
}

describe("normalizeMenus", () => {
  it("按 sort_order 升序排列", () => {
    const nodes = normalizeMenus([
      menu({ id: "b", sort_order: 2 }),
      menu({ id: "a", sort_order: 1 })
    ]);
    expect(nodes.map(node => node.id)).toEqual(["a", "b"]);
  });

  it("过滤掉 visible=false 的节点", () => {
    const nodes = normalizeMenus([menu({ id: "hidden", visible: false })]);
    expect(nodes).toHaveLength(0);
  });

  it("过滤掉 status 非 active 的节点", () => {
    const nodes = normalizeMenus([
      menu({ id: "disabled", status: "disabled" })
    ]);
    expect(nodes).toHaveLength(0);
  });

  it("保留 children 并递归处理", () => {
    const nodes = normalizeMenus([
      menu({
        id: "parent",
        path: "/parent",
        children: [menu({ id: "child", path: "/parent/child" })]
      })
    ]);
    expect(nodes[0].children).toHaveLength(1);
    expect(nodes[0].children[0].path).toBe("/parent/child");
  });

  it("没有路径也没有子节点的分组项不展示", () => {
    const nodes = normalizeMenus([menu({ id: "group", path: "" })]);
    expect(nodes).toHaveLength(0);
  });

  it("子节点被过滤后父节点仍保留", () => {
    const nodes = normalizeMenus([
      menu({
        id: "parent",
        path: "/parent",
        children: [menu({ id: "child", visible: false })]
      })
    ]);
    expect(nodes).toHaveLength(1);
    expect(nodes[0].children).toHaveLength(0);
  });
});

describe("flattenMenus", () => {
  it("把树展平成 path → 节点 的索引", () => {
    const nodes: MenuNode[] = [
      {
        id: "p",
        name: "父",
        path: "/p",
        children: [{ id: "c", name: "子", path: "/p/c", children: [] }]
      }
    ];
    const index = flattenMenus(nodes);
    expect(index.get("/p")?.id).toBe("p");
    expect(index.get("/p/c")?.id).toBe("c");
  });

  it("没有 path 的节点不进索引", () => {
    const index = flattenMenus([
      { id: "x", name: "x", path: "", children: [] }
    ]);
    expect(index.size).toBe(0);
  });
});
