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
});
