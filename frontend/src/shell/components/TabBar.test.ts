// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import TabBar from "./TabBar.vue";
import { mountShell } from "../test-utils";

const tabs = [
  { path: "/dashboard", title: "概览" },
  { path: "/system", title: "系统管理" }
];

function mountTabBar() {
  return mountShell(TabBar, {
    props: { tabs, activePath: "/dashboard" }
  });
}

describe("TabBar", () => {
  it("渲染标签、关闭按钮与前后翻页按钮", () => {
    const w = mountTabBar();
    expect(w.findAll(".ui-tabbar__tab")).toHaveLength(2);
    expect(w.findAll(".ui-tabbar__close")).toHaveLength(2);
    expect(w.findAll(".ui-tabbar__nav")).toHaveLength(2);
    expect(w.find(".ui-tabbar__scroll").attributes("role")).toBe("tablist");
  });

  it("点击标签触发 select", async () => {
    const w = mountTabBar();
    await w.findAll(".ui-tabbar__tab")[1].trigger("click");
    expect(w.emitted("select")).toEqual([["/system"]]);
  });

  it("点击关闭按钮只触发 close", async () => {
    const w = mountTabBar();
    await w.findAll(".ui-tabbar__close")[0].trigger("click");
    expect(w.emitted("close")).toEqual([["/dashboard"]]);
    expect(w.emitted("select")).toBeUndefined();
  });

  it("标签未溢出时前后按钮禁用", () => {
    const w = mountTabBar();
    const navs = w.findAll(".ui-tabbar__nav");
    expect(navs[0].attributes("disabled")).toBeDefined();
    expect(navs[1].attributes("disabled")).toBeDefined();
  });

  it("右侧溢出时启用右按钮并按一页滚动", async () => {
    const w = mountTabBar();
    const scrollEl = w.find(".ui-tabbar__scroll").element as HTMLElement;
    Object.defineProperty(scrollEl, "scrollWidth", {
      configurable: true,
      value: 600
    });
    Object.defineProperty(scrollEl, "clientWidth", {
      configurable: true,
      value: 200
    });
    scrollEl.scrollLeft = 0;
    const scrollBy = vi.fn();
    Object.defineProperty(scrollEl, "scrollBy", {
      configurable: true,
      value: scrollBy
    });
    scrollEl.dispatchEvent(new Event("scroll"));
    await nextTick();

    const navs = w.findAll(".ui-tabbar__nav");
    expect(navs[0].attributes("disabled")).toBeDefined();
    expect(navs[1].attributes("disabled")).toBeUndefined();

    await navs[1].trigger("click");
    expect(scrollBy).toHaveBeenCalledWith({ left: 176, behavior: "smooth" });
  });

  it("右键标签弹出上下文菜单，按位置禁用菜单项", async () => {
    const w = mountTabBar();
    expect(document.body.querySelector(".ui-tabbar__menu")).toBeNull();

    await w.findAll(".ui-tabbar__tab")[0].trigger("contextmenu");
    let items = menuItems();
    expect(items).toHaveLength(5);
    expect(items[0].disabled).toBe(false);
    expect(items[1].disabled).toBe(true); // 目标已是第一个，无左侧可关
    expect(items[2].disabled).toBe(false);
    expect(items[3].disabled).toBe(false);
    expect(items[4].disabled).toBe(false);

    document.body
      .querySelectorAll<HTMLElement>(".ui-tabbar__menu-item")[2]
      .click();
    await nextTick();
    expect(w.emitted("closeRight")).toEqual([["/dashboard"]]);
    expect(document.body.querySelector(".ui-tabbar__menu")).toBeNull();

    await w.findAll(".ui-tabbar__tab")[1].trigger("contextmenu");
    items = menuItems();
    expect(items[1].disabled).toBe(false);
    expect(items[2].disabled).toBe(true); // 目标已是最后一个，无右侧可关
  });

  it("点击关闭全部触发 closeAll 并收起菜单", async () => {
    const w = mountTabBar();
    await w.findAll(".ui-tabbar__tab")[0].trigger("contextmenu");
    document.body
      .querySelectorAll<HTMLElement>(".ui-tabbar__menu-item")[4]
      .click();
    await nextTick();
    expect(w.emitted("closeAll")).toEqual([[]]);
    expect(document.body.querySelector(".ui-tabbar__menu")).toBeNull();
  });

  it("不可关标签：关闭项禁用，批量项把它排除在外", async () => {
    const w = mountShell(TabBar, {
      props: {
        tabs: [
          { path: "/home", title: "首页", closable: false },
          { path: "/a", title: "A" }
        ],
        activePath: "/a"
      }
    });
    await w.findAll(".ui-tabbar__tab")[0].trigger("contextmenu");
    const items = menuItems();
    expect(items[0].disabled).toBe(true);
    expect(items[3].disabled).toBe(false); // 还有可关的 /a

    document.body
      .querySelectorAll<HTMLElement>(".ui-tabbar__menu-item")[3]
      .click();
    await nextTick();
    expect(w.emitted("closeOthers")).toEqual([["/home"]]);
  });

  it("Escape 关闭菜单", async () => {
    const w = mountTabBar();
    await w.findAll(".ui-tabbar__tab")[0].trigger("contextmenu");
    expect(document.body.querySelector(".ui-tabbar__menu")).not.toBeNull();
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await nextTick();
    expect(document.body.querySelector(".ui-tabbar__menu")).toBeNull();
  });

  it("点击菜单外关闭菜单", async () => {
    const w = mountTabBar();
    await w.findAll(".ui-tabbar__tab")[0].trigger("contextmenu");
    expect(document.body.querySelector(".ui-tabbar__menu")).not.toBeNull();
    document.body.dispatchEvent(new MouseEvent("pointerdown"));
    await nextTick();
    expect(document.body.querySelector(".ui-tabbar__menu")).toBeNull();
  });
});

function menuItems() {
  return Array.from(
    document.body.querySelectorAll<HTMLElement>(".ui-tabbar__menu-item")
  );
}
