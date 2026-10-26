// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import FormField from "./FormField.vue";
import Input from "../input/Input.vue";

describe("FormField", () => {
  it("渲染标签并关联控件", () => {
    const w = mount(FormField, { props: { label: "用户名", htmlFor: "u" } });
    expect(w.find("label").attributes("for")).toBe("u");
    expect(w.find("label").text()).toContain("用户名");
  });

  it("有 error 时渲染 role=alert 的错误文案", () => {
    const w = mount(FormField, { props: { error: "名称不能超过 24 个字符" } });
    const err = w.find(".ui-form-field__error");
    expect(err.exists()).toBe(true);
    expect(err.attributes("role")).toBe("alert");
    expect(err.text()).toBe("名称不能超过 24 个字符");
  });

  it("无 error 时不渲染错误文案元素", () => {
    expect(mount(FormField).find(".ui-form-field__error").exists()).toBe(false);
  });

  it("有 error 时根元素进入错误态", () => {
    expect(mount(FormField, { props: { error: "x" } }).classes()).toContain(
      "is-invalid"
    );
    expect(mount(FormField).classes()).not.toContain("is-invalid");
  });

  it("required 显示必填标记", () => {
    const w = mount(FormField, { props: { label: "邮箱", required: true } });
    expect(w.find(".ui-form-field__label").text()).toContain("*");
  });

  it("description 存在时渲染说明", () => {
    expect(
      mount(FormField, { props: { description: "用于登录" } })
        .find(".ui-form-field__description")
        .exists()
    ).toBe(true);
  });

  it("error 优先于 description，二者不同时渲染", () => {
    const w = mount(FormField, {
      props: { error: "填错了", description: "用于登录" }
    });
    expect(w.find(".ui-form-field__error").exists()).toBe(true);
    expect(w.find(".ui-form-field__description").exists()).toBe(false);
  });

  it("错误文案带 id，可被控件的 aria-describedby 引用", () => {
    const w = mount(FormField, { props: { error: "填错了" } });
    expect(w.find(".ui-form-field__error").attributes("id")).toBeTruthy();
  });

  it("错误态下控件自动获得 aria-describedby 与 aria-invalid", async () => {
    const w = mount({
      components: { FormField, Input },
      template: `<FormField label="邮箱" error="格式不对"><Input /></FormField>`
    });
    await flushPromises();
    const input = w.find("input");
    const errorId = w.find(".ui-form-field__error").attributes("id");
    expect(input.attributes("aria-describedby")).toBe(errorId);
    expect(input.attributes("aria-invalid")).toBe("true");
  });
});
