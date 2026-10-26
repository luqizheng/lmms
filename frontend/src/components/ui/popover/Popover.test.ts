// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { mount, type VueWrapper } from "@vue/test-utils";
import { nextTick } from "vue";
import { Popover } from "./index";

const wrappers: VueWrapper[] = [];

function mountPopover(props: Record<string, unknown> = {}) {
  const wrapper = mount(Popover, {
    props: { title: "标题", ...props },
    slots: { default: "触发器", content: "浮层内容" },
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

function dialogInDom() {
  return document.querySelector<HTMLElement>('[role="dialog"]');
}

afterEach(() => {
  while (wrappers.length) wrappers.pop()?.unmount();
  document.body.innerHTML = "";
});

describe("Popover", () => {
  it("触发器渲染为 button，初始浮层关闭", () => {
    const w = mountPopover();
    const trigger = w.find("button");
    expect(trigger.exists()).toBe(true);
    expect(trigger.attributes("aria-expanded")).toBe("false");
    expect(trigger.attributes("aria-haspopup")).toBe("dialog");
    expect(dialogInDom()).toBeNull();
  });

  it("点击触发器打开浮层，展开状态同步到 aria-expanded", async () => {
    const w = mountPopover();
    await w.find("button").trigger("click");
    await flush();

    const dialog = dialogInDom();
    expect(dialog).not.toBeNull();
    expect(dialog?.textContent).toContain("浮层内容");
    expect(w.find("button").attributes("aria-expanded")).toBe("true");
  });

  it("Esc 关闭浮层并把焦点还给触发器", async () => {
    const w = mountPopover();
    const trigger = w.find("button");
    trigger.element.focus();
    await trigger.trigger("click");
    await flush();
    expect(dialogInDom()).not.toBeNull();

    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true })
    );
    await flush();
    expect(dialogInDom()).toBeNull();
    expect(document.activeElement).toBe(trigger.element);
  });

  it("浮层为 role=dialog 并由触发器标注", async () => {
    const w = mountPopover();
    await w.find("button").trigger("click");
    await flush();
    const dialog = dialogInDom();
    const trigger = w.find("button");
    expect(dialog?.getAttribute("aria-labelledby")).toBe(
      trigger.attributes("id")
    );
  });

  it("受控 open 生效并可回写 update:open", async () => {
    const w = mountPopover({ open: true });
    await flush();
    expect(dialogInDom()).not.toBeNull();

    await w.setProps({ open: false });
    await flush();
    expect(dialogInDom()).toBeNull();
  });

  it("支持箭头，箭头渲染在浮层内", async () => {
    const w = mountPopover({ arrow: true });
    await w.find("button").trigger("click");
    await flush();
    expect(document.querySelector(".ui-popover__arrow")).not.toBeNull();
  });

  it("不传 arrow 时不渲染箭头", async () => {
    const w = mountPopover();
    await w.find("button").trigger("click");
    await flush();
    expect(document.querySelector(".ui-popover__arrow")).toBeNull();
  });

  it("触发器可访问：提供 aria-label 时输出到按钮", () => {
    const w = mountPopover({ ariaLabel: "更多信息" });
    expect(w.find("button").attributes("aria-label")).toBe("更多信息");
  });
});
