// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { Dialog } from "./index";

const source = readFileSync(resolve(__dirname, "Dialog.vue"), "utf8");

const wrappers: Array<{ unmount: () => void }> = [];

function mountDialog(props: Record<string, unknown> = {}) {
  const wrapper = mount(Dialog, {
    props: { open: true, title: "编辑部署", ...props },
    attachTo: document.body,
    slots: { default: "<p>表单内容</p>" }
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

describe("Dialog", () => {
  it("渲染标题与内容，语义为 dialog", async () => {
    const w = mountDialog({ description: "修改后立即生效" });
    await flush();
    const dialog = document.querySelector<HTMLElement>('[role="dialog"]');
    expect(dialog).not.toBeNull();
    expect(dialog?.textContent).toContain("编辑部署");
    expect(dialog?.textContent).toContain("表单内容");
    expect(dialog?.textContent).toContain("修改后立即生效");
    expect(w.exists()).toBe(true);
  });

  it("遮罩存在且内容为白卡 + shadow-lg", async () => {
    mountDialog();
    await flush();
    expect(document.querySelector(".ui-dialog__overlay")).not.toBeNull();
    expect(document.querySelector(".ui-dialog__content")).not.toBeNull();
    expect(source).toContain("var(--surface-raised)");
    expect(source).toContain("var(--shadow-lg)");
    expect(source).toContain("var(--border-strong)");
  });

  it("关闭按钮存在并请求关闭", async () => {
    const w = mountDialog({ closeLabel: "关闭" });
    await flush();
    const close = document.querySelector<HTMLElement>(".ui-dialog__close");
    expect(close).not.toBeNull();
    expect(close?.getAttribute("aria-label")).toBe("关闭");
    close?.click();
    await nextTick();
    expect(w.emitted("update:open")).toEqual([[false]]);
  });

  it("尺寸档位映射到约定类名", async () => {
    mountDialog({ size: "lg" });
    await flush();
    expect(document.querySelector(".ui-dialog--lg")).not.toBeNull();
  });

  it("浮层动画 320ms 且 reduced-motion 下仅保留 opacity", () => {
    expect(source).toContain("var(--duration-slow)");
    expect(source).toContain("prefers-reduced-motion");
    expect(source).not.toMatch(/\b320ms\b/);
  });
});
