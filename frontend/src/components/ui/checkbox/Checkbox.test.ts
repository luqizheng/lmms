// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { mount, type VueWrapper } from "@vue/test-utils";
import { nextTick } from "vue";
import { Checkbox } from "./index";

const wrappers: VueWrapper[] = [];

function mountCheckbox(props: Record<string, unknown> = {}) {
  const wrapper = mount(Checkbox, {
    props: { ariaLabel: "勾选项", ...props },
    attachTo: document.body
  });
  wrappers.push(wrapper);
  return wrapper;
}

/** 取得 checkbox 根按钮 */
function rootOf(w: ReturnType<typeof mountCheckbox>) {
  return w.find<HTMLElement>('[role="checkbox"]');
}

afterEach(() => {
  while (wrappers.length) wrappers.pop()?.unmount();
  document.body.innerHTML = "";
});

describe("Checkbox", () => {
  it("渲染为 role=checkbox，初始未选中", () => {
    const w = mountCheckbox();
    const root = rootOf(w);
    expect(root.exists()).toBe(true);
    expect(root.attributes("aria-checked")).toBe("false");
    expect(root.attributes("data-state")).toBe("unchecked");
  });

  it("点击切换选中状态并抛出 update:modelValue", async () => {
    const w = mountCheckbox();
    await rootOf(w).trigger("click");
    expect(w.emitted("update:modelValue")?.[0]).toEqual([true]);
  });

  it("受控模式下 modelValue 为真时 aria-checked=true 且渲染指示器", async () => {
    const w = mountCheckbox({ modelValue: true });
    expect(rootOf(w).attributes("aria-checked")).toBe("true");
    await nextTick();
    expect(w.find(".ui-checkbox__indicator").exists()).toBe(true);
  });

  it("未选中时不渲染指示器", () => {
    const w = mountCheckbox();
    expect(w.find(".ui-checkbox__indicator").exists()).toBe(false);
  });

  it("根节点是原生 button，键盘可达（原生按钮语义）", () => {
    const w = mountCheckbox();
    const root = rootOf(w);
    expect(root.element.tagName).toBe("BUTTON");
    expect(root.attributes("type")).toBe("button");
  });

  it("禁用态不可切换", async () => {
    const w = mountCheckbox({ disabled: true });
    const root = rootOf(w);
    expect(root.attributes("data-disabled")).toBeDefined();
    expect(root.attributes("aria-disabled")).toBe("true");
    await root.trigger("click");
    expect(w.emitted("update:modelValue")).toBeUndefined();
  });

  it("半选状态输出 aria-checked=mixed", () => {
    const w = mountCheckbox({ modelValue: "indeterminate" });
    expect(rootOf(w).attributes("aria-checked")).toBe("mixed");
    expect(rootOf(w).attributes("data-state")).toBe("indeterminate");
  });

  it("可见标签通过 for/id 与勾选框关联，点击标签即切换", async () => {
    const w = mountCheckbox({ label: "记住我" });
    const root = rootOf(w);
    const label = w.find("label");
    expect(label.text()).toBe("记住我");
    expect(label.attributes("for")).toBe(root.attributes("id"));
    expect(root.attributes("aria-labelledby")).toBe(label.attributes("id"));

    await label.trigger("click");
    expect(w.emitted("update:modelValue")?.[0]).toEqual([true]);
  });

  it("空格键切换勾选状态", async () => {
    const w = mountCheckbox();
    const root = rootOf(w);
    root.element.focus();
    await root.trigger("keydown", { key: " " });
    expect(w.emitted("update:modelValue")?.[0]).toEqual([true]);
  });

  it("禁用态不响应空格键", async () => {
    const w = mountCheckbox({ disabled: true });
    await rootOf(w).trigger("keydown", { key: " " });
    expect(w.emitted("update:modelValue")).toBeUndefined();
  });

  it("必填时输出 aria-required", () => {
    const w = mountCheckbox({ required: true });
    expect(rootOf(w).attributes("aria-required")).toBe("true");
  });
});
