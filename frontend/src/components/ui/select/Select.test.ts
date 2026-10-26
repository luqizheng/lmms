// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { mount, type VueWrapper } from "@vue/test-utils";
import { h, nextTick } from "vue";
import { Select } from "./index";

const options = [
  { value: "alpha", label: "Alpha" },
  { value: "beta", label: "Beta" },
  { value: "gamma", label: "Gamma", disabled: true }
];

/** 等待 reka-ui 的 Presence / Portal 渲染完成 */
async function flush() {
  await nextTick();
  await new Promise(resolve => setTimeout(resolve, 0));
  await nextTick();
}

const wrappers: VueWrapper[] = [];

function mountSelect(props: Record<string, unknown> = {}) {
  const wrapper = mount(Select, {
    props: { options, ariaLabel: "选择项", ...props },
    attachTo: document.body
  });
  wrappers.push(wrapper);
  return wrapper;
}

function triggerOf(w: ReturnType<typeof mountSelect>) {
  return w.find<HTMLElement>('[role="combobox"]').element;
}

function optionsInDom() {
  return Array.from(document.querySelectorAll<HTMLElement>('[role="option"]'));
}

afterEach(() => {
  // 先卸载再清空 body：避免浮层还在时被外部移除导致 Vue 更新报错
  while (wrappers.length) wrappers.pop()?.unmount();
  document.body.innerHTML = "";
});

describe("Select", () => {
  it("触发器渲染为 combobox，初始收起", () => {
    const w = mountSelect();
    const trigger = w.find('[role="combobox"]');
    expect(trigger.exists()).toBe(true);
    expect(trigger.attributes("aria-expanded")).toBe("false");
    expect(trigger.attributes("data-state")).toBe("closed");
  });

  it("未选中时展示占位文案", () => {
    const w = mountSelect({ placeholder: "请选择" });
    expect(w.find('[role="combobox"]').text()).toContain("请选择");
  });

  it("打开后浮层为 listbox，选项为 option", async () => {
    const w = mountSelect();
    await w.find('[role="combobox"]').trigger("pointerdown");
    await flush();
    expect(document.querySelector('[role="listbox"]')).not.toBeNull();
    expect(optionsInDom().length).toBe(3);
  });

  it("键盘 ArrowDown 打开浮层并预高亮首项（Phase 2 §5.2）", async () => {
    const w = mountSelect();
    const trigger = triggerOf(w);
    trigger.focus();
    trigger.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true })
    );
    await flush();
    expect(trigger.getAttribute("aria-expanded")).toBe("true");
    const highlighted = document.querySelector("[data-highlighted]");
    expect(highlighted?.textContent).toContain("Alpha");
  });

  it("上下键移动高亮，Enter 选中并回写 modelValue", async () => {
    const w = mountSelect();
    const trigger = triggerOf(w);
    trigger.focus();
    trigger.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true })
    );
    await flush();

    // 打开后焦点在浮层内的选项上，方向键在选项之间移动高亮
    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true })
    );
    await flush();
    expect(document.querySelector("[data-highlighted]")?.textContent).toContain(
      "Beta"
    );

    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Enter", bubbles: true })
    );
    await flush();
    expect(w.emitted("update:modelValue")?.[0]).toEqual(["beta"]);

    await w.setProps({ modelValue: "beta" });
    await flush();
    expect(w.find('[role="combobox"]').text()).toContain("Beta");
  });

  it("Esc 关闭浮层并把焦点还给触发器", async () => {
    const w = mountSelect();
    const trigger = triggerOf(w);
    trigger.focus();
    trigger.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true })
    );
    await flush();
    expect(trigger.getAttribute("aria-expanded")).toBe("true");

    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true })
    );
    await flush();
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    expect(document.activeElement).toBe(trigger);
  });

  it("选中项标记 aria-selected 与 checked 状态", async () => {
    const w = mountSelect({ modelValue: "alpha" });
    await w.find('[role="combobox"]').trigger("pointerdown");
    await flush();
    const alpha = optionsInDom().find(option =>
      option.textContent?.includes("Alpha")
    );
    expect(alpha?.getAttribute("aria-selected")).toBe("true");
    expect(alpha?.getAttribute("data-state")).toBe("checked");
  });

  it("禁用选项不可选中", async () => {
    const w = mountSelect();
    await w.find('[role="combobox"]').trigger("pointerdown");
    await flush();
    const gamma = optionsInDom().find(option =>
      option.textContent?.includes("Gamma")
    );
    expect(gamma?.getAttribute("aria-disabled")).toBe("true");
    expect(gamma?.getAttribute("data-disabled")).toBeDefined();

    gamma?.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
    await flush();
    expect(w.emitted("update:modelValue")).toBeUndefined();
  });

  it("disabled 时触发器不可用", () => {
    const w = mountSelect({ disabled: true });
    const trigger = w.find('[role="combobox"]');
    expect(trigger.attributes("disabled")).toBeDefined();
    expect(trigger.attributes("data-disabled")).toBeDefined();
  });

  it("invalid 时输出 aria-invalid", () => {
    const w = mountSelect({ invalid: true });
    expect(w.find('[role="combobox"]').attributes("aria-invalid")).toBe("true");
    expect(
      mountSelect().find('[role="combobox"]').attributes("aria-invalid")
    ).toBeUndefined();
  });

  it("尺寸档位映射到约定的类名", () => {
    expect(
      mountSelect({ size: "sm" }).find('[role="combobox"]').classes()
    ).toContain("ui-select__trigger--sm");
    expect(
      mountSelect({ size: "md" }).find('[role="combobox"]').classes()
    ).toContain("ui-select__trigger--md");
  });

  it("item 插槽可自定义选项内容", async () => {
    const w = mount(Select, {
      props: { options, ariaLabel: "选择项" },
      slots: {
        item: (slotProps: { option: { label: string } }) =>
          h("em", { class: "custom" }, slotProps.option.label)
      },
      attachTo: document.body
    });
    wrappers.push(w);
    await w.find('[role="combobox"]').trigger("pointerdown");
    await flush();
    expect(document.querySelector(".custom")?.textContent).toContain("Alpha");
  });
});
