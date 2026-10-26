// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Badge from "./Badge.vue";

describe("Badge", () => {
  it("默认 neutral 变体", () => {
    const w = mount(Badge);
    expect(w.classes()).toContain("ui-badge");
    expect(w.classes()).toContain("ui-badge--neutral");
  });

  it("六种变体均可渲染", () => {
    for (const v of [
      "neutral",
      "success",
      "warning",
      "danger",
      "info",
      "accent"
    ]) {
      expect(mount(Badge, { props: { variant: v } }).classes()).toContain(
        `ui-badge--${v}`
      );
    }
  });

  it("根元素为 span，插槽内容可见", () => {
    const w = mount(Badge, { slots: { default: "运行中" } });
    expect(w.element.tagName).toBe("SPAN");
    expect(w.text()).toBe("运行中");
  });
});
