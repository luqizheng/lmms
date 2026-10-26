// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { Sheet } from "./index";

const source = readFileSync(resolve(__dirname, "Sheet.vue"), "utf8");

const wrappers: Array<{ unmount: () => void }> = [];

function mountSheet(props: Record<string, unknown> = {}) {
  const wrapper = mount(Sheet, {
    props: { open: true, title: "导航", ...props },
    attachTo: document.body,
    slots: { default: "<p>菜单</p>" }
  });
  wrappers.push(wrapper);
  return wrapper;
}

/** reka-ui 的 Portal 需要一拍才把内容挂到 body */
async function flush() {
  await nextTick();
  await new Promise(resolve => setTimeout(resolve, 0));
  await nextTick();
}

afterEach(() => {
  while (wrappers.length) wrappers.pop()?.unmount();
  document.body.innerHTML = "";
});

describe("Sheet", () => {
  it("渲染为左侧抽屉，语义为 dialog", async () => {
    mountSheet();
    await flush();
    const dialog = document.querySelector<HTMLElement>('[role="dialog"]');
    expect(dialog).not.toBeNull();
    expect(dialog?.classList.contains("ui-sheet")).toBe(true);
    expect(dialog?.getAttribute("data-side")).toBe("left");
    expect(dialog?.textContent).toContain("菜单");
  });

  it("宽度 240px 来自 token，遮罩为暖黑纱", async () => {
    mountSheet();
    await flush();
    expect(source).toContain("var(--size-sidebar)");
    expect(source).not.toMatch(/\b240px\b/);
    expect(document.querySelector(".ui-sheet__overlay")).not.toBeNull();
  });

  it("关闭按钮请求关闭", async () => {
    const w = mountSheet({ closeLabel: "关闭" });
    await flush();
    const close = document.querySelector<HTMLElement>(".ui-sheet__close");
    expect(close?.getAttribute("aria-label")).toBe("关闭");
    close?.click();
    await nextTick();
    expect(w.emitted("update:open")).toEqual([[false]]);
  });

  it("滑入动画 240ms 且 reduced-motion 下降级", () => {
    expect(source).toContain("var(--duration-medium)");
    expect(source).toContain("prefers-reduced-motion");
    expect(source).not.toMatch(/\b240ms\b/);
  });
});
