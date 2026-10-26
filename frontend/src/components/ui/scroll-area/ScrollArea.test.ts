// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { ScrollArea } from "./index";

const source = readFileSync(resolve(__dirname, "ScrollArea.vue"), "utf8");

describe("ScrollArea", () => {
  it("渲染滚动容器与可聚焦的视口", () => {
    const w = mount(ScrollArea, { slots: { default: "<p>内容</p>" } });

    const root = w.element as HTMLElement;
    expect(root.classList.contains("ui-scroll-area")).toBe(true);

    const viewport = w.find("[data-reka-scroll-area-viewport]");
    expect(viewport.exists()).toBe(true);
    // 视口可聚焦，键盘可滚动
    expect(viewport.attributes("tabindex")).toBe("0");
    expect(viewport.text()).toContain("内容");
  });

  it("渲染滚动条与滑块，并标注方向", () => {
    const w = mount(ScrollArea, { slots: { default: "<p>内容</p>" } });

    const scrollbar = w.find(".ui-scroll-area__scrollbar");
    expect(scrollbar.exists()).toBe(true);
    expect(scrollbar.attributes("data-orientation")).toBe("vertical");
    expect(w.find(".ui-scroll-area__thumb").exists()).toBe(true);
  });

  it("horizontal 方向把滚动条标记为 horizontal", () => {
    const w = mount(ScrollArea, {
      props: { orientation: "horizontal" },
      slots: { default: "<p>内容</p>" }
    });
    expect(
      w.find(".ui-scroll-area__scrollbar").attributes("data-orientation")
    ).toBe("horizontal");
  });

  it("滚动条未出现时处于 hidden 状态（浮层式浮出）", () => {
    const w = mount(ScrollArea, { slots: { default: "<p>内容</p>" } });
    expect(w.find(".ui-scroll-area__scrollbar").attributes("data-state")).toBe(
      "hidden"
    );
  });

  it("滚动条宽度 8px、滑块配色与淡入时长全部来自 token", () => {
    // 8px = --space-2
    expect(source).toContain("var(--space-2)");
    expect(source).toContain("var(--border-strong)");
    expect(source).toContain("var(--border-data)");
    expect(source).toContain("var(--duration-fast)");
    expect(source).not.toMatch(/8px/);
  });

  it("滚动条与滑块使用 :deep() 命中 reka-ui 内部元素", () => {
    expect(source).toContain(":deep(.ui-scroll-area__viewport)");
    expect(source).toContain(":deep(.ui-scroll-area__scrollbar)");
    expect(source).toContain(":deep(.ui-scroll-area__thumb)");
  });
});
