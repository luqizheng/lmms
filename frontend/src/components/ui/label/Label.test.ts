// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Label from "./Label.vue";

describe("Label", () => {
  it("透传 for 属性", () => {
    expect(mount(Label, { props: { for: "x" } }).attributes("for")).toBe("x");
  });

  it("required 时显示必填标记", () => {
    const w = mount(Label, { props: { required: true } });
    expect(w.text()).toContain("*");
    expect(w.classes()).toContain("is-required");
  });

  it("非 required 时无必填标记", () => {
    expect(mount(Label).text()).not.toContain("*");
  });

  it("必填标记对读屏隐藏，仅装饰", () => {
    const w = mount(Label, { props: { required: true } });
    expect(w.find(".ui-label__required").attributes("aria-hidden")).toBe(
      "true"
    );
  });
});
