import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import {
  extractPathList,
  deleteChildren,
  buildHierarchyTree,
  getNodeByUniqueId,
  appendFieldByUniqueId,
  handleTree
} from "./tree";

describe("extractPathList", () => {
  it("collects uniqueId from flat tree", () => {
    const tree = [{ uniqueId: 1 }, { uniqueId: 2 }, { uniqueId: 3 }];
    expect(extractPathList(tree)).toEqual([1, 2, 3]);
  });

  it("recursively calls itself on children but discards the result (source bug)", () => {
    const tree = [
      {
        uniqueId: 1,
        children: [
          { uniqueId: 2 },
          { uniqueId: 3, children: [{ uniqueId: 4 }] }
        ]
      },
      { uniqueId: 5 }
    ];
    expect(extractPathList(tree)).toEqual([1, 5]);
  });

  it("returns [] for empty / non-array input", () => {
    expect(extractPathList([])).toEqual([]);
    expect(extractPathList(null as any)).toEqual([]);
    expect(extractPathList("not-array" as any)).toEqual([]);
  });
});

describe("deleteChildren", () => {
  it("removes children when length is 1 and adds hierarchy fields", () => {
    const tree: any = [
      { id: 10, children: [{ id: 11 }] },
      { id: 20, children: [{ id: 21 }, { id: 22 }] }
    ];
    const out = deleteChildren(tree);
    // 第一个节点单子 → children 被删
    expect(out[0].children).toBeUndefined();
    // 第二个节点多子 → children 保留
    expect(out[1].children).toHaveLength(2);
    // 字段被加
    expect(out[0].id).toBe(0);
    expect(out[0].parentId).toBe(null);
    expect(out[0].uniqueId).toBe(0);
    expect(out[0].pathList).toEqual([0]);
    // 第二个的子节点 uniqueId 应包含父级 path
    expect(out[1].children[0].uniqueId).toBe("1-0");
  });

  it("returns [] for non-array input", () => {
    expect(deleteChildren(null as any)).toEqual([]);
  });
});

describe("buildHierarchyTree", () => {
  it("adds id/parentId/pathList without deleting children", () => {
    const tree: any = [
      {
        children: [{ children: [{ id: 99 }] }, { id: 88 }]
      }
    ];
    const out = buildHierarchyTree(tree);
    expect(out[0].id).toBe(0);
    expect(out[0].parentId).toBe(null);
    expect(out[0].pathList).toEqual([0]);
    expect(out[0].children).toHaveLength(2);
    // 子节点保留 children（不像 deleteChildren 会删）
    expect(out[0].children[0].children).toHaveLength(1);
    // 没有 uniqueId 字段
    expect(out[0].uniqueId).toBeUndefined();
  });
});

describe("getNodeByUniqueId", () => {
  const tree: any = [
    {
      uniqueId: "0",
      children: [
        { uniqueId: "0-0", children: [{ uniqueId: "0-0-0", name: "deep" }] }
      ]
    },
    { uniqueId: "1" }
  ];

  it("finds top-level node", () => {
    expect(getNodeByUniqueId(tree, "1")).toEqual({ uniqueId: "1" });
  });

  it("finds deep node by BFS recursion", () => {
    const found = getNodeByUniqueId(tree, "0-0-0");
    expect(found.name).toBe("deep");
  });

  it("returns [] when not found", () => {
    expect(getNodeByUniqueId(tree, "nope")).toEqual([]);
  });
});

describe("appendFieldByUniqueId", () => {
  it("appends fields to matching node", () => {
    const tree: any = [
      {
        uniqueId: "a",
        children: [{ uniqueId: "b" }]
      }
    ];
    const out = appendFieldByUniqueId(tree, "b", { highlight: true, count: 3 });
    expect(out[0].children[0].highlight).toBe(true);
    expect(out[0].children[0].count).toBe(3);
    // 父节点不应被改
    expect(out[0].highlight).toBeUndefined();
  });

  it("skips when fields is not a plain object", () => {
    const tree: any = [{ uniqueId: "x" }];
    const out = appendFieldByUniqueId(tree, "x", "not-object" as any);
    // fields 不是对象，target 节点无新增字段
    expect(out[0].highlight).toBeUndefined();
    // array 也是 toString.call !== "[object Object]"
    const out2 = appendFieldByUniqueId(tree, "x", [1, 2] as any);
    expect(out2[0].foo).toBeUndefined();
  });
});

describe("handleTree", () => {
  it("builds tree from flat list with default id/parentId/children", () => {
    const flat = [
      { id: 1, parentId: 0, name: "root" },
      { id: 2, parentId: 1, name: "child-a" },
      { id: 3, parentId: 1, name: "child-b" },
      { id: 4, parentId: 2, name: "grand" }
    ];
    const tree = handleTree(flat);
    expect(tree).toHaveLength(1);
    expect(tree[0].id).toBe(1);
    expect(tree[0].children).toHaveLength(2);
    const childA = tree[0].children.find((c: any) => c.id === 2);
    expect(childA.children).toHaveLength(1);
    expect(childA.children[0].name).toBe("grand");
  });

  it("supports custom field names", () => {
    const flat = [
      { _id: 1, _pid: 0 },
      { _id: 2, _pid: 1 },
      { _id: 3, _pid: 1 }
    ];
    const tree = handleTree(flat, "_id", "_pid");
    expect(tree).toHaveLength(1);
    expect(tree[0]._id).toBe(1);
    expect(tree[0].children).toHaveLength(2);
  });

  it("returns [] for non-array / empty", () => {
    expect(handleTree([])).toEqual([]);
    expect(handleTree(null as any)).toEqual([]);
  });
});

describe("console.warn noise (non-array inputs)", () => {
  beforeEach(() =>
    vi.spyOn(console, "warn").mockImplementation(() => undefined)
  );
  afterEach(() => vi.restoreAllMocks());

  it("suppresses warns for invalid inputs", () => {
    extractPathList(null as any);
    deleteChildren(undefined as any);
    buildHierarchyTree("nope" as any);
    getNodeByUniqueId(0 as any, 0);
    appendFieldByUniqueId(null as any, "x", {});
    handleTree("not-array" as any);
    expect(console.warn).toHaveBeenCalled();
  });
});
