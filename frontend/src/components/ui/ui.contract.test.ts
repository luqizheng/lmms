// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * 组件样式契约测试。
 *
 * happy-dom 无法计算 scoped CSS 的最终样式，因此对"CSS 里必须出现的声明"
 * 采用源码契约断言 —— 它拦住的是"删掉一段样式，类名断言仍然全绿"这类回归。
 */
const UI_DIR = resolve(process.cwd(), "src/components/ui");

function source(...parts: string[]): string {
  return readFileSync(resolve(UI_DIR, ...parts), "utf8");
}

describe("组件样式契约（Phase 1R 数值必须来自 token）", () => {
  it("Button 的尺寸 / 缩放 / 动效时长来自 token", () => {
    const s = source("button", "Button.vue");
    expect(s).toContain("var(--size-control-md)");
    expect(s).toContain("var(--size-control-sm)");
    expect(s).toContain("var(--size-control-lg)");
    expect(s).toContain("var(--size-icon-button)");
    expect(s).toContain("var(--scale-press)");
    expect(s).toContain("var(--duration-spinner)");
    expect(s).toContain("var(--size-spinner)");
    expect(s).not.toMatch(/scale\(0\.98\)/);
    expect(s).not.toMatch(/\b\d+ms\b(?![^{]*\bvar\()/);
  });

  it("Button 主按钮带 §6.1 规定的顶部 inset 高光", () => {
    expect(source("button", "Button.vue")).toContain(
      "var(--surface-highlight)"
    );
  });

  it("Button 在 reduced-motion 下取消按压缩放", () => {
    const s = source("button", "Button.vue");
    expect(s).toContain("prefers-reduced-motion");
    expect(s).toMatch(/transform:\s*none/);
  });

  it("Skeleton 的 shimmer 周期来自 token 且 reduced-motion 下静止", () => {
    const s = source("skeleton", "Skeleton.vue");
    expect(s).toContain("var(--duration-shimmer)");
    expect(s).toContain("prefers-reduced-motion");
    expect(s).toContain("animation: none");
  });

  it("Input / Textarea 的控件高度来自 token", () => {
    const input = source("input/Input.vue");
    expect(input).toContain("var(--size-control-md)");
    expect(input).toContain("var(--size-control-sm)");
    expect(input).not.toMatch(/height:\s*3[26]px/);

    const textarea = source("textarea/Textarea.vue");
    expect(textarea).toContain("var(--size-control-md)");
    expect(textarea).not.toMatch(/height:\s*3[26]px/);
  });

  it("Input 错误态常驻 danger 光晕（§6.2：描边 + 光晕同时存在）", () => {
    const s = source("input", "Input.vue");
    const invalidBlock = s.slice(s.indexOf(".ui-input.is-invalid {"));
    expect(invalidBlock).toContain("var(--danger-focus-glow)");
  });

  it("Table 行高 / 表头高 / Badge 高来自 token", () => {
    expect(source("table", "TableRow.vue")).toContain("var(--size-row)");
    expect(source("table", "TableHeader.vue")).toContain(
      "var(--size-header-cell)"
    );
    expect(source("badge", "Badge.vue")).toContain("var(--size-badge)");
  });

  it("Table 表头字重为 §6.7 规定的 450 档", () => {
    const s = source("table", "TableHeader.vue");
    expect(s).toContain("var(--weight-caption)");
    expect(s).not.toContain("var(--weight-regular)");
  });

  it("Table 行 hover 时分隔线升到 border-default（§6.7 括注）", () => {
    expect(source("table", "TableRow.vue")).toContain("var(--border-default)");
  });

  it("Table 的 caption 不依赖尚未引入的全局 .sr-only", () => {
    const s = source("table", "Table.vue");
    expect(s).not.toContain('class="sr-only"');
    expect(s).toContain("ui-sr-only");
  });

  it("Input / Textarea 自带焦点环，不依赖全局 base.css", () => {
    for (const file of ["input/Input.vue", "textarea/Textarea.vue"]) {
      const s = source(file);
      expect(s).toContain("var(--size-focus-ring)");
      expect(s).toContain("var(--input-focus-glow)");
      expect(s).toContain("var(--size-input-halo)");
    }
  });

  it("Button 的 secondary / ghost 加载指示用 --text-secondary（Phase 2 §6.1）", () => {
    const s = source("button", "Button.vue");
    expect(s).toContain("var(--text-secondary)");
  });
});
