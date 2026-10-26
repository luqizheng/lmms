// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Card from "./Card.vue";

describe("Card", () => {
  it("渲染 header / default / footer 三个插槽", () => {
    const w = mount(Card, {
      slots: {
        header: "<h3>标题</h3>",
        default: "<p>正文</p>",
        footer: "<span>底部</span>"
      }
    });
    expect(w.find(".ui-card__header").exists()).toBe(true);
    expect(w.find(".ui-card__body").exists()).toBe(true);
    expect(w.find(".ui-card__footer").exists()).toBe(true);
  });

  it("interactive 附加交互态类", () => {
    expect(mount(Card).classes()).not.toContain("is-interactive");
    expect(mount(Card, { props: { interactive: true } }).classes()).toContain(
      "is-interactive"
    );
  });

  it("无 footer 插槽时不渲染 footer 容器", () => {
    expect(mount(Card).find(".ui-card__footer").exists()).toBe(false);
  });

  it("根元素为 section 地标", () => {
    expect(mount(Card).element.tagName).toBe("SECTION");
  });
});
