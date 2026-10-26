// @vitest-environment happy-dom
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const PRISM_DIR = resolve(process.cwd(), "src/styles/prism");
const css = readFileSync(resolve(PRISM_DIR, "tokens.css"), "utf8");
const brandPath = resolve(PRISM_DIR, "tokens.brand.css");
const brandCss = existsSync(brandPath) ? readFileSync(brandPath, "utf8") : "";

function token(name: string, source = css): string {
  const m = source.match(new RegExp(`--${name}:\\s*([^;]+);`));
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

/* ── Phase 1R 附录 CSS 变量速查：变量名 → 期望值 ───────────────────────
 * 值比较忽略空白（prettier 会规范化 rgba 内的空格），保证"值等价"可被断言。
 * ---------------------------------------------------------------------- */

const APPENDIX_VALUES: Record<string, string> = {
  // Surface
  "surface-sunken": "#F0F0EC",
  "surface-base": "#F7F7F4",
  "surface-raised": "#FFFFFF",
  "surface-overlay": "#FFFFFF",
  "surface-hover-veil": "rgba(23,24,28,0.04)",
  "surface-active-veil": "rgba(23,24,28,0.07)",
  "surface-ink": "#17181C",
  "ink-hover-veil": "rgba(255,255,255,0.08)",
  "ink-active-veil": "rgba(255,255,255,0.12)",
  "focus-glow-ink": "rgba(46,91,255,0.32)",
  // Text
  "text-primary": "#17181C",
  "text-secondary": "#52535B",
  "text-tertiary": "#6A6B73",
  "text-disabled": "#9C9DA5",
  "text-inverse": "#FFFFFF",
  // Border
  "border-subtle": "#EDEDE9",
  "border-default": "#E3E3DE",
  "border-strong": "#D4D4CE",
  "border-data": "#8A8B93",
  "border-accent-glow": "rgba(46,91,255,0.3)",
  // Accent
  accent: "#2E5BFF",
  "accent-hover": "#3D67FF",
  "accent-pressed": "#1F44D6",
  "accent-deep": "#2447CC",
  "accent-subtle": "#EBF0FF",
  "accent-wash": "rgba(46,91,255,0.08)",
  "accent-gradient": "linear-gradient(180deg,#2E5BFF 0%,#2750E6 100%)",
  "accent-contrast-text": "#FFFFFF",
  // Semantic
  success: "#15803D",
  "success-strong": "#166534",
  "success-subtle": "#ECF8F0",
  warning: "#B45309",
  "warning-strong": "#92400E",
  "warning-subtle": "#FCF3E3",
  danger: "#DC2626",
  "danger-strong": "#B91C1C",
  "danger-subtle": "#FDEDED",
  info: "#0F766E",
  "info-strong": "#115E59",
  "info-subtle": "#E6F4F2",
  // Shadow
  "shadow-xs": "0 1px 2px rgba(23,24,28,0.04)",
  "shadow-sm": "0 1px 2px rgba(23,24,28,0.05),0 2px 6px rgba(23,24,28,0.05)",
  "shadow-md": "0 2px 6px rgba(23,24,28,0.06),0 12px 24px rgba(23,24,28,0.09)",
  "shadow-lg": "0 4px 12px rgba(23,24,28,0.08),0 24px 48px rgba(23,24,28,0.14)",
  "shadow-accent":
    "0 1px 2px rgba(30,64,175,0.24),0 4px 12px rgba(46,91,255,0.22)",
  "shadow-accent-lg":
    "0 2px 4px rgba(30,64,175,0.28),0 8px 20px rgba(46,91,255,0.28)",
  // Radius
  "radius-xs": "4px",
  "radius-sm": "6px",
  "radius-md": "8px",
  "radius-lg": "12px",
  "radius-xl": "16px",
  "radius-full": "999px",
  // Duration / Easing
  "duration-instant": "80ms",
  "duration-fast": "150ms",
  "duration-medium": "240ms",
  "duration-slow": "320ms",
  "ease-decelerate": "cubic-bezier(0.16,1,0.3,1)",
  "ease-standard": "cubic-bezier(0.4,0,0.2,1)",
  // Font
  "font-ui":
    '"Inter","MiSans","PingFang SC","HarmonyOS Sans SC","Noto Sans SC","Microsoft YaHei UI","Segoe UI",system-ui,sans-serif',
  "font-mono":
    '"JetBrains Mono","SF Mono","Cascadia Code","Roboto Mono","Consolas","Noto Sans Mono CJK SC",monospace'
};

const APPENDIX_TOKENS = Object.keys(APPENDIX_VALUES);

/** 去掉所有空白，用于值等价比较（prettier 会规范化 rgba 内的空格） */
function normalize(value: string): string {
  return value.replace(/\s+/g, "");
}

describe("PRISM token 契约", () => {
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
