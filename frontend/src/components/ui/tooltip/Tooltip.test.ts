// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { mount, type VueWrapper } from "@vue/test-utils";
import { nextTick } from "vue";
import { Tooltip } from "./index";

const wrappers: VueWrapper[] = [];

function mountTooltip(props: Record<string, unknown> = {}) {
  const wrapper = mount(Tooltip, {
    props: { content: "提示文案", ...props },
    slots: { default: "触发器" },
    attachTo: document.body
  });
  wrappers.push(wrapper);
  return wrapper;
}

/** 等待 reka-ui 的 Presence / Portal 渲染完成 */
async function flush() {
  await nextTick();
  await new Promise(resolve => setTimeout(resolve, 0));
  await nextTick();
}

function contentInDom() {
  return document.querySelector<HTMLElement>('[role="tooltip"]');
}

afterEach(() => {
  while (wrappers.length) wrappers.pop()?.unmount();
  document.body.innerHTML = "";
});

describe("Tooltip", () => {
  it("触发器渲染为 button，默认不展示浮层", () => {
    const w = mountTooltip();
    const trigger = w.find("button");
    expect(trigger.exists()).toBe(true);
    expect(trigger.attributes("aria-describedby")).toBeUndefined();
    expect(contentInDom()).toBeNull();
  });

  it("聚焦触发器时展示浮层并建立 aria-describedby 关联", async () => {
    const w = mountTooltip();
    const trigger = w.find("button");
    trigger.element.dispatchEvent(new FocusEvent("focus", { bubbles: false }));
    await flush();

    const content = contentInDom();
    expect(content).not.toBeNull();
    expect(content?.textContent).toContain("提示文案");
    expect(trigger.attributes("aria-describedby")).toBe(content?.id);
  });

  it("失焦后浮层关闭", async () => {
    const w = mountTooltip();
    const trigger = w.find("button");
    trigger.element.dispatchEvent(new FocusEvent("focus", { bubbles: false }));
    await flush();
    expect(contentInDom()).not.toBeNull();

    trigger.element.dispatchEvent(new FocusEvent("blur", { bubbles: false }));
    await flush();
    expect(contentInDom()).toBeNull();
  });

  it("浮层内容使用 role=tooltip 且带 data-state", async () => {
    const w = mountTooltip();
    w.find("button").element.dispatchEvent(
      new FocusEvent("focus", { bubbles: false })
    );
    await flush();
    // reka-ui：role=tooltip 落在浮层内的读屏专用元素上，
    // data-state 落在浮层容器上
    expect(contentInDom()).not.toBeNull();
    const container = document.querySelector<HTMLElement>(
      ".ui-tooltip__content"
    );
    expect(container?.getAttribute("data-state")).toBe("instant-open");
  });

  it("disabled 时不展示浮层", async () => {
    const w = mountTooltip({ disabled: true });
    w.find("button").element.dispatchEvent(
      new FocusEvent("focus", { bubbles: false })
    );
    await flush();
    expect(contentInDom()).toBeNull();
  });

  it("支持受控 open 与 update:open 事件", async () => {
    const w = mountTooltip({ open: true });
    await flush();
    expect(contentInDom()).not.toBeNull();

    await w.setProps({ open: false });
    await flush();
    expect(contentInDom()).toBeNull();
  });

  it("触发器可访问：提供 aria-label 时输出到按钮", () => {
    const w = mountTooltip({ ariaLabel: "更多信息" });
    expect(w.find("button").attributes("aria-label")).toBe("更多信息");
  });
});
