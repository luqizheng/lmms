// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Textarea from "./Textarea.vue";

describe("Textarea", () => {
  it("双向绑定 modelValue", async () => {
    const w = mount(Textarea, { props: { modelValue: "a" } });
    expect((w.element as HTMLTextAreaElement).value).toBe("a");
    await w.setValue("b");
    expect(w.emitted("update:modelValue")?.[0]).toEqual(["b"]);
  });

  it("默认 rows 为 4", () => {
    expect(mount(Textarea).attributes("rows")).toBe("4");
  });

  it("invalid 时附加错误描边类", () => {
    expect(mount(Textarea, { props: { invalid: true } }).classes()).toContain(
      "is-invalid"
    );
  });

  it("disabled 透传到原生元素", () => {
    expect(
      mount(Textarea, { props: { disabled: true } }).attributes("disabled")
    ).toBeDefined();
  });
});
