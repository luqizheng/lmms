// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "../test-utils";
import DataTable from "./DataTable.vue";

const columns = [
  { label: "用户名", prop: "username" },
  { label: "状态", prop: "status" },
  { label: "创建时间", prop: "created_at", numeric: true }
];

describe("DataTable", () => {
  it("渲染表头与数据行", () => {
    const w = mountShell(DataTable, {
      props: {
        columns,
        rows: [{ username: "admin", status: "启用", created_at: "2026-01-01" }]
      }
    });
    expect(w.findAll("th").length).toBe(3);
    expect(w.text()).toContain("admin");
    expect(w.text()).toContain("启用");
  });

  it("无数据时给出空态文案", () => {
    const w = mountShell(DataTable, { props: { columns, rows: [], emptyText: "还没有记录" } });
    expect(w.find(".ui-data-table__empty").text()).toBe("还没有记录");
  });

  it("加载中时显示骨架而不是旧数据", () => {
    const w = mountShell(DataTable, {
      props: { columns, rows: [{ username: "admin" }], loading: true }
    });
    expect(w.find(".ui-skeleton").exists()).toBe(true);
    expect(w.text()).not.toContain("admin");
  });

  it("查询条件提交时回传参数", async () => {
    const w = mountShell(DataTable, {
      props: {
        columns,
        rows: [],
        query: [{ prop: "keyword", placeholder: "搜索用户名" }]
      }
    });
    const input = w.find("input");
    await input.setValue("ada");
    await w.find("form").trigger("submit");
    await nextTick();
    expect(w.emitted("search")?.[0]?.[0]).toMatchObject({ keyword: "ada" });
  });

  it("数值列带等宽数字类", () => {
    const w = mountShell(DataTable, {
      props: { columns, rows: [{ username: "a", created_at: "1" }] }
    });
    expect(w.findAll("td.is-numeric").length).toBe(1);
  });

  it("分页变化时回传页码", async () => {
    const w = mountShell(DataTable, {
      props: { columns, rows: [], total: 100, pageSize: 10, page: 1 }
    });
    const buttons = w.findAll("button");
    const third = buttons.find(b => b.text() === "3");
    await third?.trigger("click");
    await nextTick();
    expect(w.emitted("update:page")?.[0]).toEqual([3]);
  });
});
