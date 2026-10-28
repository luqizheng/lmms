// @vitest-environment happy-dom
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const PRISM_DIR = resolve(process.cwd(), "src/styles/prism");
const css = readFileSync(resolve(PRISM_DIR, "tokens.css"), "utf8");
const brandPath = resolve(PRISM_DIR, "tokens.brand.css");
const brandCss = existsSync(brandPath) ? readFileSync(brandPath, "utf8") : "";

/**
 * PRISM 规范文档（仓库内权威副本）。
 * 向上找 `docs/prism/`，兼容从仓库根或 frontend/ 两种 cwd 启动。
 */
function findPrismSpec(): string {
  const candidates = [
    resolve(process.cwd(), "..", "docs", "prism", "phase1-visual-direction.md"),
    resolve(process.cwd(), "docs", "prism", "phase1-visual-direction.md")
  ];
  const found = candidates.find(candidate => existsSync(candidate));
  if (!found) {
    throw new Error(`找不到 PRISM 规范文档：${candidates.join(" / ")}`);
  }
  return found;
}

/**
 * 解析规范附录「CSS 变量速查」里的 `:root` 变量表。
 *
 * 规范改了而 `tokens.css` 没改 → 下面的"逐个等价"断言变红。
 * 这就是散文契约与代码契约的焊点：**不再手工誊抄一份附录**。
 */
function parseAppendixTokens(): Record<string, string> {
  const doc = readFileSync(findPrismSpec(), "utf8");
  const appendixIndex = doc.indexOf("## 附录");
  if (appendixIndex === -1) throw new Error("规范里没有找到附录小节");
  const fence = doc.slice(appendixIndex).match(/```css\n([\s\S]*?)\n```/);
  if (!fence) throw new Error("规范附录里没有找到 css 围栏块");

  const values: Record<string, string> = {};
  for (const match of fence[1].matchAll(/--([a-z0-9-]+)\s*:\s*([^;]+);/g)) {
    values[match[1]] = match[2].trim();
  }
  return values;
}

/** 去掉 CSS 注释：tokens.brand.css 里的示例写在注释里，不能当成真实覆盖值 */
function stripComments(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//g, "");
}

function token(name: string, source = css): string {
  const m = stripComments(source).match(new RegExp(`--${name}:\\s*([^;]+);`));
  return m ? m[1].trim() : "";
}

/** 取 token 值里的全部 hex（渐变有两个端点） */
function hexesOf(name: string, source = css): string[] {
  return [...token(name, source).matchAll(/#[0-9a-fA-F]{6}\b/g)].map(m => m[0]);
}

/* ── WCAG 相对亮度与对比度 ───────────────────────────────────────────── */

function srgbToLinear(channel: number): number {
  const c = channel / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

function luminance(hex: string): number {
  const n = parseInt(hex.slice(1), 16);
  const r = srgbToLinear((n >> 16) & 0xff);
  const g = srgbToLinear((n >> 8) & 0xff);
  const b = srgbToLinear(n & 0xff);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: string, b: string): number {
  const l1 = luminance(a);
  const l2 = luminance(b);
  const [hi, lo] = l1 > l2 ? [l1, l2] : [l2, l1];
  return (hi + 0.05) / (lo + 0.05);
}

/** 有效 token 中的第一个 hex（品牌覆盖优先），用于对比度计算 */
function firstHex(name: string): string {
  const brand = hexesOf(name, brandCss)[0];
  return brand ?? hexesOf(name)[0] ?? "";
}

/* ── Phase 1R 附录 CSS 变量速查：期望值直接取自规范文档 ───────────────
 * 不再手工誊抄一份副本：规范改了而 tokens.css 没改，下面的等价断言会变红。
 * 规范文档：docs/prism/phase1-visual-direction.md（仓库内权威副本）
 * ---------------------------------------------------------------------- */

const APPENDIX_VALUES = parseAppendixTokens();
const APPENDIX_TOKENS = Object.keys(APPENDIX_VALUES);

/**
 * 值等价比较的规范化：
 *   1. 去全部空白 —— prettier 会规范化 `rgba()` 内外的空格；
 *   2. 小数尾零归一 —— `0.30` 与 `0.3` 数值等价；
 *   3. font-family 里标识符的引号可选 —— `Consolas` 与 `"Consolas"` 等价。
 * 规范与 tokens.css 是"逐字转录"关系，但工具链（prettier）会做上述规范化，
 * 所以比较的是**语义等价**，不是字节相同。
 */
function normalize(value: string): string {
  return value
    .replace(/\s+/g, "")
    .replace(/\.(\d*?)0+(?=[,)])/g, ".$1")
    .replace(/"([A-Za-z][\w-]*)"/g, "$1");
}

describe("PRISM token 契约", () => {
  it("附录解析成功：拿到规范里的变量表（防解析静默失败）", () => {
    const parsed = parseAppendixTokens();
    expect(Object.keys(parsed).length).toBeGreaterThan(50);
    // 关键变量必须解析到，否则说明文档结构变了、解析器失效
    for (const key of ["--accent", "--surface-base", "--text-primary"]) {
      const name = key.replace(/^--/, "");
      expect(parsed[name]).toBeTruthy();
    }
  });

  it("附录 CSS 变量速查的每个变量都在 tokens.css 中定义", () => {
    const missing = APPENDIX_TOKENS.filter(name => token(name) === "");
    expect(missing).toEqual([]);
  });
  it("附录变量的值与 Phase 1R 速查逐个等价（忽略空白）", () => {
    const drifted: string[] = [];
    for (const [name, expected] of Object.entries(APPENDIX_VALUES)) {
      const actual = token(name);
      if (normalize(actual) !== normalize(expected)) {
        drifted.push(`${name}: 期望 ${expected}，实际 ${actual || "(缺失)"}`);
      }
    }
    expect(drifted).toEqual([]);
  });

  it("附录之外的变量必须落在可追溯来源的命名族内", () => {
    const defined = [...css.matchAll(/--([a-z0-9-]+)\s*:/g)].map(m => m[1]);
    const appendixSet = new Set(APPENDIX_TOKENS);
    const orphans = defined.filter(
      name =>
        !appendixSet.has(name) &&
        !/^(font-size|line-height|weight|tracking|space|size|duration-|scale-|surface-|focus-|input-focus|danger-focus|skeleton-|accent-|border-|hairline$)/.test(
          name
        )
    );
    expect(orphans).toEqual([]);
  });

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

  it("字阶四族齐全（Phase 1R §3.2）", () => {
    const levels = [
      "display",
      "h1",
      "h2",
      "h3",
      "h4",
      "body",
      "body-strong",
      "small",
      "caption",
      "mono-num"
    ];
    for (const level of levels) {
      expect(token(`font-size-${level}`), `缺少 --font-size-${level}`).not.toBe(
        ""
      );
      expect(
        token(`line-height-${level}`),
        `缺少 --line-height-${level}`
      ).not.toBe("");
      expect(token(`tracking-${level}`), `缺少 --tracking-${level}`).not.toBe(
        ""
      );
    }
  });

  it("字重档覆盖 Phase 1R §3.2 用到的全部档位", () => {
    expect(token("weight-regular")).toBe("400");
    expect(token("weight-medium")).toBe("500");
    expect(token("weight-semibold")).toBe("600");
    expect(token("weight-bold")).toBe("650");
    expect(token("weight-caption")).toBe("450");
    expect(token("weight-body-strong")).toBe("550");
  });

  it("尺寸与动效补充 token 齐全（Phase 1R §4.3 / §6 / §7.1）", () => {
    for (const name of [
      "size-control-sm",
      "size-control-md",
      "size-control-lg",
      "size-row",
      "size-header-cell",
      "size-badge",
      "size-spinner",
      "size-focus-ring",
      "size-focus-halo",
      "size-input-halo",
      "size-indicator",
      "size-menu-min-width",
      "size-menu-padding",
      "size-icon-xs",
      "size-icon-lg",
      "size-badge",
      "size-switch-track",
      "size-switch-thumb",
      "size-switch-thumb-press",
      "size-checkbox",
      "accent-disabled",
      "duration-switch",
      "border-1",
      "border-2",
      "hairline",
      "scale-menu-enter",
      "scale-press",
      "duration-shimmer",
      "duration-spinner",
      "scale-press"
    ]) {
      expect(token(name), `缺少 token --${name}`).not.toBe("");
    }
  });

  it("字阶数值与 Phase 1R §3.2 一致", () => {
    expect(token("font-size-body")).toBe("15px");
    expect(token("line-height-body")).toBe("26px");
    expect(token("font-size-small")).toBe("13px");
    expect(token("line-height-small")).toBe("20px");
    expect(token("font-size-caption")).toBe("12px");
    expect(token("line-height-caption")).toBe("18px");
    expect(token("tracking-caption")).toBe("0.025em");
  });

  it("每个变量声明完整闭合，不留悬空冒号", () => {
    const leftover = css.replace(/--[a-z0-9-]+\s*:[^;]+;/g, "");
    expect(leftover).not.toMatch(/--[a-z0-9-]+\s*:/);
  });

  /* ── WCAG 对比度：按有效 token 值动态计算，换肤后依然有效 ─────────── */

  it("正文文字在全部 surface 上对比度 ≥ 4.5:1（Phase 1R §2.7）", () => {
    const surfaces = ["surface-raised", "surface-base", "surface-sunken"];
    for (const text of ["text-primary", "text-secondary", "text-tertiary"]) {
      for (const surface of surfaces) {
        const ratio = contrast(firstHex(text), firstHex(surface));
        expect(
          ratio,
          `${text} on ${surface} 对比度 ${ratio.toFixed(2)}:1 < 4.5:1`
        ).toBeGreaterThanOrEqual(4.5);
      }
    }
  });

  it("accent 作为链接文字在画布与白卡上 ≥ 4.5:1", () => {
    for (const surface of ["surface-raised", "surface-base"]) {
      const ratio = contrast(firstHex("accent"), firstHex(surface));
      expect(
        ratio,
        `accent on ${surface} 对比度 ${ratio.toFixed(2)}:1 < 4.5:1`
      ).toBeGreaterThanOrEqual(4.5);
    }
  });

  it("accent 实底配白字 ≥ 4.5:1（含渐变两端）", () => {
    const stops = [...hexesOf("accent"), ...hexesOf("accent", brandCss)];
    expect(stops.length).toBeGreaterThan(0);
    for (const stop of stops) {
      const ratio = contrast("#FFFFFF", stop);
      expect(
        ratio,
        `白字 on ${stop} 对比度 ${ratio.toFixed(2)}:1 < 4.5:1`
      ).toBeGreaterThanOrEqual(4.5);
    }
  });

  it("主按钮在 hover / pressed 态的白字对比度 ≥ 4.5:1（Phase 1R §2.7）", () => {
    for (const name of ["accent-hover", "accent-pressed"]) {
      const ratio = contrast("#FFFFFF", firstHex(name));
      expect(
        ratio,
        `白字 on ${name} 对比度 ${ratio.toFixed(2)}:1 < 4.5:1`
      ).toBeGreaterThanOrEqual(4.5);
    }
  });

  it("tint 底上的文字用 accent-deep，≥ 4.5:1", () => {
    const ratio = contrast(firstHex("accent-deep"), firstHex("accent-subtle"));
    expect(
      ratio,
      `accent-deep on accent-subtle 对比度 ${ratio.toFixed(2)}:1`
    ).toBeGreaterThanOrEqual(4.5);
  });

  it("语义色 strong 档在各自 subtle 底上 ≥ 4.5:1", () => {
    for (const semantic of ["success", "warning", "danger", "info"]) {
      const ratio = contrast(
        firstHex(`${semantic}-strong`),
        firstHex(`${semantic}-subtle`)
      );
      expect(
        ratio,
        `${semantic}-strong on ${semantic}-subtle 对比度 ${ratio.toFixed(2)}:1`
      ).toBeGreaterThanOrEqual(4.5);
    }
  });

  it("墨色面板上的反色文字 ≥ 4.5:1", () => {
    const ratio = contrast(firstHex("text-inverse"), firstHex("surface-ink"));
    expect(
      ratio,
      `inverse on ink 对比度 ${ratio.toFixed(2)}:1`
    ).toBeGreaterThanOrEqual(4.5);
  });

  it("品牌覆盖文件不得引入未定义变量", () => {
    const defined = new Set(
      [...css.matchAll(/--([a-z0-9-]+)\s*:/g)].map(m => m[1])
    );
    const used = [...brandCss.matchAll(/var\(\s*--([a-z0-9-]+)/g)].map(
      m => m[1]
    );
    const unknown = used.filter(name => !defined.has(name));
    expect(unknown).toEqual([]);
  });
});
