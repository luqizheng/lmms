// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from "./index";

describe("Table", () => {
  it("渲染为真 table 结构并带 caption", () => {
    const w = mount(Table, {
      props: { caption: "最近部署" },
      slots: { default: "<tbody><tr><td>x</td></tr></tbody>" }
    });
    // 外层为横向滚动包裹层（Phase 2 §4.2：≥640 表格横向滚动）
    expect(w.element.tagName).toBe("DIV");
    expect(w.find("table").element.tagName).toBe("TABLE");
    expect(w.find("caption").exists()).toBe(true);
    expect(w.find("caption").text()).toBe("最近部署");
  });

  it("数值单元格带 mono 等宽数字类", () => {
    const w = mount(TableCell, { props: { numeric: true } });
    expect(w.classes()).toContain("is-numeric");
    expect(mount(TableCell).classes()).not.toContain("is-numeric");
  });

  it("选中行带选中态类", () => {
    const w = mount(TableRow, { props: { selected: true } });
    expect(w.classes()).toContain("is-selected");
  });

  it("表头默认 scope=col", () => {
    expect(mount(TableHeader).attributes("scope")).toBe("col");
  });

  it("可排序表头带 is-sortable 类", () => {
    expect(
      mount(TableHeader, { props: { sortable: true } }).classes()
    ).toContain("is-sortable");
    expect(mount(TableHeader).classes()).not.toContain("is-sortable");
  });

  it("thead / tbody 语义正确", () => {
    expect(mount(TableHead).element.tagName).toBe("THEAD");
    expect(mount(TableBody).element.tagName).toBe("TBODY");
  });

  it("表头单元格语义为 th，数据单元格为 td", () => {
    expect(mount(TableHeader).element.tagName).toBe("TH");
    expect(mount(TableCell).element.tagName).toBe("TD");
    expect(mount(TableRow).element.tagName).toBe("TR");
  });

  it("排序状态输出 aria-sort（Phase 2 §5.3）", () => {
    expect(
      mount(TableHeader, { props: { sort: "ascending" } }).attributes(
        "aria-sort"
      )
    ).toBe("ascending");
    expect(
      mount(TableHeader, { props: { sort: "descending" } }).attributes(
        "aria-sort"
      )
    ).toBe("descending");
    expect(mount(TableHeader).attributes("aria-sort")).toBeUndefined();
  });

  it("可排序表头内含可聚焦按钮，键盘可达（Phase 2 §9.4）", () => {
    const w = mount(TableHeader, { props: { sortable: true } });
    expect(w.find("button").exists()).toBe(true);
    expect(w.find("button").attributes("type")).toBe("button");
  });
});
