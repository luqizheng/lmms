// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { mount, type VueWrapper } from "@vue/test-utils";
import { h, nextTick } from "vue";
import { Tabs } from "./index";

const tabs = [
  { value: "overview", label: "Overview" },
  { value: "logs", label: "Logs" },
  { value: "audit", label: "Audit", disabled: true }
];

async function flush() {
  await nextTick();
  await new Promise(resolve => setTimeout(resolve, 0));
  await nextTick();
}

const wrappers: VueWrapper[] = [];

function mountTabs(props: Record<string, unknown> = {}) {
  const wrapper = mount(Tabs, {
    props: { tabs, ariaLabel: "详情页签", ...props },
    slots: {
      panel: (slotProps: { tab: { value: string; label: string } }) =>
        h("div", { class: "panel-body" }, slotProps.tab.value)
    },
    attachTo: document.body
  });
  wrappers.push(wrapper);
  return wrapper;
}

afterEach(() => {
  // 先卸载再清空 body：避免浮层还在时被外部移除导致 Vue 更新报错
  while (wrappers.length) wrappers.pop()?.unmount();
  document.body.innerHTML = "";
});

describe("Tabs", () => {
  it("渲染 tablist 与 tab 语义", () => {
    const w = mountTabs();
    const list = w.find('[role="tablist"]');
    expect(list.exists()).toBe(true);
    const triggers = w.findAll('[role="tab"]');
    expect(triggers.length).toBe(3);
    expect(triggers[0].text()).toContain("Overview");
  });

  it("未指定 modelValue 时默认选中第一个页签", () => {
    const w = mountTabs();
    const triggers = w.findAll('[role="tab"]');
    expect(triggers[0].attributes("aria-selected")).toBe("true");
    expect(triggers[0].attributes("data-state")).toBe("active");
    expect(triggers[1].attributes("aria-selected")).toBe("false");
  });

  it("当前项按设计文档口径：accent-subtle 底 + accent-deep 字 + 2px accent 指示条", () => {
    const s = readFileSync(resolve(__dirname, "Tabs.vue"), "utf8");
    expect(s).toContain("var(--accent-subtle)");
    expect(s).toContain("var(--accent-deep)");
    expect(s).toContain("var(--size-indicator)");
  });

  it("点击页签切换并回写 modelValue", async () => {
    const w = mountTabs({ modelValue: "overview" });
    await w.findAll('[role="tab"]')[1].trigger("mousedown", { button: 0 });
    await flush();
    expect(w.emitted("update:modelValue")?.[0]).toEqual(["logs"]);
  });

  it("面板为 tabpanel 且由对应页签标注", async () => {
    const w = mountTabs({ modelValue: "overview" });
    const panel = w.find('[role="tabpanel"]');
    expect(panel.exists()).toBe(true);
    expect(panel.attributes("data-state")).toBe("active");
    const trigger = w.findAll('[role="tab"]')[0];
    expect(panel.attributes("aria-labelledby")).toBe(trigger.attributes("id"));
  });

  it("切换 modelValue 后面板内容随之更新", async () => {
    const w = mountTabs({ modelValue: "overview" });
    expect(w.find(".panel-body").text()).toBe("overview");
    await w.setProps({ modelValue: "logs" });
    await flush();
    expect(w.find(".panel-body").text()).toBe("logs");
    expect(w.findAll('[role="tab"]')[1].attributes("aria-selected")).toBe(
      "true"
    );
  });

  it("键盘方向键在页签之间移动并激活", async () => {
    const w = mountTabs({ modelValue: "overview" });
    const first = w.findAll('[role="tab"]')[0].element as HTMLElement;
    first.focus();
    // reka-ui 的方向键监听在焦点落到页签后才绑定
    await flush();
    first.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true })
    );
    await flush();
    expect(w.emitted("update:modelValue")?.[0]).toEqual(["logs"]);
  });

  it("禁用页签不响应点击", async () => {
    const w = mountTabs({ modelValue: "overview" });
    const disabled = w.findAll('[role="tab"]')[2];
    expect(disabled.attributes("disabled")).toBeDefined();
    expect(disabled.attributes("data-disabled")).toBeDefined();
    await disabled.trigger("mousedown", { button: 0 });
    await flush();
    expect(w.emitted("update:modelValue")).toBeUndefined();
  });

  it("tab 插槽可自定义页签文案", () => {
    const w = mount(Tabs, {
      props: { tabs, ariaLabel: "详情页签" },
      slots: {
        tab: (slotProps: { tab: { label: string } }) =>
          h("span", { class: "custom-tab" }, slotProps.tab.label)
      }
    });
    wrappers.push(w);
    expect(w.find(".custom-tab").text()).toContain("Overview");
  });
});
