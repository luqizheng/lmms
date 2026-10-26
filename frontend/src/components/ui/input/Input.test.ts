// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Input from "./Input.vue";

describe("Input", () => {
  it("双向绑定 modelValue", async () => {
    const w = mount(Input, { props: { modelValue: "a" } });
    expect((w.element as HTMLInputElement).value).toBe("a");
    await w.setValue("b");
    expect(w.emitted("update:modelValue")?.[0]).toEqual(["b"]);
  });

  it("invalid 时附加错误描边类", () => {
    expect(mount(Input).classes()).not.toContain("is-invalid");
    expect(mount(Input, { props: { invalid: true } }).classes()).toContain(
      "is-invalid"
    );
  });

  it("disabled 透传到原生元素", () => {
    expect(
      mount(Input, { props: { disabled: true } }).attributes("disabled")
    ).toBeDefined();
  });

  it("尺寸映射到约定类名", () => {
    expect(mount(Input, { props: { size: "sm" } }).classes()).toContain(
      "ui-input--sm"
    );
    expect(mount(Input, { props: { size: "lg" } }).classes()).toContain(
      "ui-input--lg"
    );
    expect(mount(Input).classes()).toContain("ui-input--md");
  });

  it("invalid 时输出 aria-invalid=true", () => {
    expect(
      mount(Input, { props: { invalid: true } }).attributes("aria-invalid")
    ).toBe("true");
    expect(mount(Input).attributes("aria-invalid")).toBeUndefined();
  });
});
