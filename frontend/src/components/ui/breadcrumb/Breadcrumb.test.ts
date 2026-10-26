// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { Breadcrumb } from "./index";

const items = [{ label: "工作区", to: "/workspace" }, { label: "概览" }];

describe("Breadcrumb", () => {
  it("渲染 nav > ol > li 语义结构", () => {
    const w = mount(Breadcrumb, { props: { items } });
    expect(w.element.tagName).toBe("NAV");
    expect(w.find("ol").exists()).toBe(true);
    expect(w.findAll("li").length).toBe(2);
  });

  it("最后一项标记为当前页", () => {
    const w = mount(Breadcrumb, { props: { items } });
    const list = w.findAll("li");
    expect(list[1].attributes("aria-current")).toBe("page");
    expect(list[0].attributes("aria-current")).toBeUndefined();
  });

  it("可点击项渲染为链接并携带 href", () => {
    const w = mount(Breadcrumb, { props: { items } });
    const link = w.find("a");
    expect(link.exists()).toBe(true);
    expect(link.attributes("href")).toBe("/workspace");
  });

  it("无 to 的项不渲染链接", () => {
    const w = mount(Breadcrumb, { props: { items } });
    expect(w.findAll("a").length).toBe(1);
  });
});
