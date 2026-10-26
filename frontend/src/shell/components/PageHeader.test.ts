// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mountShell } from "../test-utils";
import PageHeader from "./PageHeader.vue";

describe("PageHeader", () => {
  it("渲染标题与副标题", () => {
    const w = mountShell(PageHeader, {
      props: { title: "缺陷", subtitle: "项目 A" }
    });
    expect(w.find(".ui-page-header__title").text()).toBe("缺陷");
    expect(w.find(".ui-page-header__subtitle").text()).toBe("项目 A");
  });

  it("back 为真时显示返回按钮并回传 back", async () => {
    const w = mountShell(PageHeader, { props: { title: "缺陷", back: true } });
    const back = w.find(".ui-page-header__back");
    expect(back.exists()).toBe(true);
    expect(back.attributes("aria-label")).toBe("返回");
    await back.trigger("click");
    expect(w.emitted("back")).toBeTruthy();
  });

  it("默认不显示返回按钮", () => {
    const w = mountShell(PageHeader, { props: { title: "缺陷" } });
    expect(w.find(".ui-page-header__back").exists()).toBe(false);
  });

  it("actions 插槽渲染在右侧", () => {
    const w = mountShell(PageHeader, {
      props: { title: "缺陷" },
      slots: { actions: "<button class='act'>新建</button>" }
    });
    expect(w.find(".ui-page-header__actions .act").exists()).toBe(true);
  });
});
