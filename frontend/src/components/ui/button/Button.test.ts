// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Button from "./Button.vue";

describe("Button", () => {
  it("默认渲染 primary 变体与 md 尺寸", () => {
    const w = mount(Button);
    expect(w.classes()).toContain("ui-button");
    expect(w.classes()).toContain("ui-button--primary");
    expect(w.classes()).toContain("ui-button--md");
  });

  it("透传 disabled 属性到原生 button", () => {
    const w = mount(Button, { props: { disabled: true } });
    expect(w.attributes("disabled")).toBeDefined();
  });

  it("loading 时禁用按钮并标记 aria-busy", () => {
    const w = mount(Button, { props: { loading: true } });
    expect(w.attributes("disabled")).toBeDefined();
    expect(w.attributes("aria-busy")).toBe("true");
    expect(w.classes()).toContain("is-loading");
  });

  it("非 loading 时不设置 aria-busy", () => {
    const w = mount(Button);
    expect(w.attributes("aria-busy")).toBeUndefined();
  });

  it("type 默认 button，可覆盖为 submit", () => {
    expect(mount(Button).attributes("type")).toBe("button");
    expect(
      mount(Button, { props: { type: "submit" } }).attributes("type")
    ).toBe("submit");
  });

  it("variant 与 size 全部映射到约定类名", () => {
    expect(
      mount(Button, { props: { variant: "secondary" } }).classes()
    ).toContain("ui-button--secondary");
    expect(mount(Button, { props: { variant: "ghost" } }).classes()).toContain(
      "ui-button--ghost"
    );
    expect(mount(Button, { props: { size: "sm" } }).classes()).toContain(
      "ui-button--sm"
    );
    expect(mount(Button, { props: { size: "lg" } }).classes()).toContain(
      "ui-button--lg"
    );
    expect(mount(Button, { props: { size: "icon" } }).classes()).toContain(
      "ui-button--icon"
    );
  });
});
