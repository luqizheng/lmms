// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { mount, type VueWrapper } from "@vue/test-utils";
import { nextTick } from "vue";
import { Switch } from "./index";

const wrappers: VueWrapper[] = [];

function mountSwitch(props: Record<string, unknown> = {}) {
  const wrapper = mount(Switch, {
    props: { ariaLabel: "开关", ...props },
    attachTo: document.body
  });
  wrappers.push(wrapper);
  return wrapper;
}

/** 取得 switch 根按钮 */
function rootOf(w: ReturnType<typeof mountSwitch>) {
  return w.find<HTMLElement>('[role="switch"]');
}

afterEach(() => {
  while (wrappers.length) wrappers.pop()?.unmount();
  document.body.innerHTML = "";
});

describe("Switch", () => {
  it("渲染为 role=switch，初始为关闭态", () => {
    const w = mountSwitch();
    const root = rootOf(w);
    expect(root.exists()).toBe(true);
    expect(root.attributes("aria-checked")).toBe("false");
    expect(root.attributes("data-state")).toBe("unchecked");
  });

  it("点击切换状态并抛出 update:modelValue", async () => {
    const w = mountSwitch();
    await rootOf(w).trigger("click");
    expect(w.emitted("update:modelValue")?.[0]).toEqual([true]);
  });

  it("受控模式下 modelValue 为真时 aria-checked=true", () => {
    const w = mountSwitch({ modelValue: true });
    expect(rootOf(w).attributes("aria-checked")).toBe("true");
    expect(rootOf(w).attributes("data-state")).toBe("checked");
  });

  it("键盘 Enter 可以切换", async () => {
    const w = mountSwitch();
    const root = rootOf(w);
    root.element.focus();
    await root.trigger("keydown", { key: "Enter" });
    expect(w.emitted("update:modelValue")?.[0]).toEqual([true]);
  });

  it("禁用态不可切换", async () => {
    const w = mountSwitch({ disabled: true });
    const root = rootOf(w);
    expect(root.attributes("data-disabled")).toBeDefined();
    expect(root.attributes("disabled")).toBeDefined();
    await root.trigger("click");
    expect(w.emitted("update:modelValue")).toBeUndefined();
  });

  it("滑块随开关状态变化 data-state", async () => {
    const w = mountSwitch();
    const thumb = w.find(".ui-switch__thumb");
    expect(thumb.attributes("data-state")).toBe("unchecked");
    await w.setProps({ modelValue: true });
    await nextTick();
    expect(w.find(".ui-switch__thumb").attributes("data-state")).toBe(
      "checked"
    );
  });

  it("提供读屏专用文本（sr-only）说明当前状态", async () => {
    const w = mountSwitch({ stateLabel: { on: "已开启", off: "已关闭" } });
    expect(w.find(".ui-sr-only").text()).toBe("已关闭");
    await w.setProps({ modelValue: true });
    await nextTick();
    expect(w.find(".ui-sr-only").text()).toBe("已开启");
  });

  it("必填时输出 aria-required", () => {
    const w = mountSwitch({ required: true });
    expect(rootOf(w).attributes("aria-required")).toBe("true");
  });

  it("可见标签通过 for/id 与开关关联", () => {
    const w = mountSwitch({ label: "开启通知" });
    const root = rootOf(w);
    const label = w.find("label");
    expect(label.text()).toBe("开启通知");
    expect(label.attributes("for")).toBe(root.attributes("id"));
    expect(root.attributes("aria-labelledby")).toBe(label.attributes("id"));
  });
});
