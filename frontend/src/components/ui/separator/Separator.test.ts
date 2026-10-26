// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { Separator } from "./index";

const source = readFileSync(resolve(__dirname, "Separator.vue"), "utf8");

describe("Separator", () => {
  it("默认渲染为 role=separator 的语义分隔线", () => {
    const w = mount(Separator);
    const el = w.element as HTMLElement;
    expect(el.getAttribute("role")).toBe("separator");
    expect(el.getAttribute("data-orientation")).toBe("horizontal");
  });

  it("horizontal 方向不输出冗余的 aria-orientation", () => {
    const w = mount(Separator, { props: { orientation: "horizontal" } });
    expect(w.element.getAttribute("aria-orientation")).toBeNull();
  });

  it("vertical 方向输出 aria-orientation=vertical", () => {
    const w = mount(Separator, { props: { orientation: "vertical" } });
    expect(w.element.getAttribute("aria-orientation")).toBe("vertical");
    expect(w.element.getAttribute("data-orientation")).toBe("vertical");
  });

  it("decorative 分隔线退化为 role=none，不进入读屏语义树", () => {
    const w = mount(Separator, { props: { decorative: true } });
    expect(w.element.getAttribute("role")).toBe("none");
    expect(w.element.getAttribute("aria-orientation")).toBeNull();
  });

  it("默认使用弱档描边 --border-subtle，strong 档升到 --border-default", () => {
    expect(source).toContain("var(--border-subtle)");
    expect(source).toContain("var(--border-default)");
    expect(source).toMatch(/background:\s*var\(--border-subtle\)/);
    expect(source).toMatch(/background:\s*var\(--border-default\)/);
  });

  it("横线与竖线各自的尺寸来自 token", () => {
    expect(source).toContain("var(--hairline)");
    expect(source).toMatch(/width:\s*var\(--hairline\)/);
    expect(source).toMatch(/height:\s*var\(--hairline\)/);
  });
});
