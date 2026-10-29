import { describe, expect, it } from "vitest";
import { planTabClose, type TabItem } from "./tab-close";

const tabs: TabItem[] = [
  { path: "/a", title: "A" },
  { path: "/b", title: "B" },
  { path: "/c", title: "C" },
  { path: "/d", title: "D" }
];

describe("planTabClose", () => {
  it("单标签关闭：左邻优先回退", () => {
    const { remaining, fallback } = planTabClose(
      tabs,
      tab => tab.path === "/c",
      "/c"
    );
    expect(remaining.map(tab => tab.path)).toEqual(["/a", "/b", "/d"]);
    expect(fallback?.path).toBe("/b");
  });

  it("最左标签被关：向右回退", () => {
    const { fallback } = planTabClose(tabs, tab => tab.path === "/a", "/a");
    expect(fallback?.path).toBe("/b");
  });

  it("激活标签未被关时不给回退目标", () => {
    const { remaining, fallback } = planTabClose(
      tabs,
      tab => tab.path === "/a",
      "/c"
    );
    expect(remaining.map(tab => tab.path)).toEqual(["/b", "/c", "/d"]);
    expect(fallback).toBeUndefined();
  });

  it("关闭左侧：跳过不可关标签，回退最近的幸存者", () => {
    const pinned: TabItem[] = [
      { path: "/home", title: "首页", closable: false },
      { path: "/a", title: "A" },
      { path: "/b", title: "B" },
      { path: "/c", title: "C" }
    ];
    const { remaining, fallback } = planTabClose(
      pinned,
      (tab, index) => index < 3 && tab.closable !== false,
      "/b"
    );
    expect(remaining.map(tab => tab.path)).toEqual(["/home", "/c"]);
    expect(fallback?.path).toBe("/home");
  });

  it("全部关闭且激活被关：无回退目标", () => {
    const { remaining, fallback } = planTabClose(
      tabs,
      tab => tab.closable !== false,
      "/b"
    );
    expect(remaining).toEqual([]);
    expect(fallback).toBeUndefined();
  });

  it("激活标签不在列表中：正常过滤、不跳转", () => {
    const { remaining, fallback } = planTabClose(
      tabs,
      tab => tab.path === "/a",
      "/zzz"
    );
    expect(remaining.map(tab => tab.path)).toEqual(["/b", "/c", "/d"]);
    expect(fallback).toBeUndefined();
  });
});
