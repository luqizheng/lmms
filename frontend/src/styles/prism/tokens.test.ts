// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const css = readFileSync(resolve(process.cwd(), "src/styles/prism/tokens.css"), "utf8");

function token(name: string): string {
  const m = css.match(new RegExp(`--${name}:\\s*([^;]+);`));
  return m ? m[1].trim() : "";
}

describe("PRISM token 契约", () => {
  it("品牌关键色值被锁定", () => {
    expect(token("accent")).toBe("#2E5BFF");
    expect(token("accent-hover")).toBe("#3D67FF");
    expect(token("accent-pressed")).toBe("#1F44D6");
    expect(token("accent-deep")).toBe("#2447CC");
    expect(token("accent-subtle")).toBe("#EBF0FF");
  });

  it("surface 三层暖灰阶梯被锁定", () => {
    expect(token("surface-sunken")).toBe("#F0F0EC");
    expect(token("surface-base")).toBe("#F7F7F4");
    expect(token("surface-raised")).toBe("#FFFFFF");
    expect(token("surface-ink")).toBe("#17181C");
  });

  it("文字四档 + 反色被锁定", () => {
    expect(token("text-primary")).toBe("#17181C");
    expect(token("text-secondary")).toBe("#52535B");
    expect(token("text-tertiary")).toBe("#6A6B73");
    expect(token("text-disabled")).toBe("#9C9DA5");
    expect(token("text-inverse")).toBe("#FFFFFF");
  });

  it("圆角 / 间距 / 时长 / 缓动阶梯齐全", () => {
    for (const name of [
      "radius-xs",
      "radius-sm",
      "radius-md",
      "radius-lg",
      "radius-xl",
      "radius-full",
      "space-1",
      "space-2",
      "space-3",
      "space-4",
      "space-5",
      "space-6",
      "space-8",
      "space-10",
      "space-12",
      "space-16",
      "space-20",
      "space-24",
      "duration-instant",
      "duration-fast",
      "duration-medium",
      "duration-slow",
      "ease-decelerate",
      "ease-standard"
    ]) {
      expect(token(name), `缺少 token --${name}`).not.toBe("");
    }
  });

  it("阴影六档齐全", () => {
    for (const name of [
      "shadow-xs",
      "shadow-sm",
      "shadow-md",
      "shadow-lg",
      "shadow-accent",
      "shadow-accent-lg"
    ]) {
      expect(token(name), `缺少 token --${name}`).not.toBe("");
    }
  });

  it("字阶十级齐全（Phase 1R §3.2）", () => {
    for (const name of [
      "text-display",
      "text-h1",
      "text-h2",
      "text-h3",
      "text-h4",
      "text-body",
      "text-body-strong",
      "text-small",
      "text-caption",
      "text-mono-num"
    ]) {
      expect(token(name), `缺少 token --${name}`).not.toBe("");
    }
  });

  it("每个变量声明完整闭合，不留悬空冒号", () => {
    const leftover = css.replace(/--[a-z0-9-]+\s*:[^;]+;/g, "");
    expect(leftover).not.toMatch(/--[a-z0-9-]+\s*:/);
    expect(css.match(/--[a-z0-9-]+\s*:/g)?.length ?? 0).toBeGreaterThan(90);
  });
});
