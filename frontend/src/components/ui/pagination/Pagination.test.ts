// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { Pagination } from "./index";

function mountPagination(props: Record<string, unknown> = {}) {
  return mount(Pagination, {
    props: { page: 2, total: 100, pageSize: 10, ...props }
  });
}

describe("Pagination", () => {
  it("渲染分页导航并带 aria-label", () => {
    const w = mountPagination();
    expect(w.element.tagName).toBe("NAV");
    expect(w.attributes("aria-label")).toBe("分页");
  });

  it("当前页标记 aria-current=page", () => {
    const w = mountPagination();
    const current = w.findAll("[aria-current='page']");
    expect(current.length).toBe(1);
    expect(current[0].text()).toBe("2");
  });

  it("上一页 / 下一页按钮存在并在边界时禁用", async () => {
    const w = mountPagination({ page: 1 });
    const buttons = w.findAll("button");
    const prev = buttons[0];
    expect(prev.attributes("disabled")).toBeDefined();

    const w2 = mountPagination({ page: 10 });
    const buttons2 = w2.findAll("button");
    expect(buttons2[buttons2.length - 1].attributes("disabled")).toBeDefined();
  });

  it("点击页码回传目标页", async () => {
    const w = mountPagination();
    const buttons = w.findAll("button");
    const third = buttons.find(b => b.text() === "3");
    await third?.trigger("click");
    await nextTick();
    expect(w.emitted("update:page")?.[0]).toEqual([3]);
  });

  it("总页数由 total / pageSize 推导", () => {
    const w = mountPagination({ total: 95, pageSize: 10 });
    const labels = w.findAll("button").map(b => b.text());
    expect(labels).toContain("10");
  });
});
