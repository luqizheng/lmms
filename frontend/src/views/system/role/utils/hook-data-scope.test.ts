import { describe, expect, it } from "vitest";
import { dataScopeText, dataScopeToNumber } from "./data-scope";

describe("dataScopeText", () => {
  it("returns Chinese label for numeric codes 0-3", () => {
    expect(dataScopeText(0)).toBe("仅本人");
    expect(dataScopeText(1)).toBe("本部门");
    expect(dataScopeText(2)).toBe("本部门及以下");
    expect(dataScopeText(3)).toBe("全部");
  });

  it("returns Chinese label for string enum keys", () => {
    expect(dataScopeText("self")).toBe("仅本人");
    expect(dataScopeText("dept")).toBe("本部门");
    expect(dataScopeText("dept_and_below")).toBe("本部门及以下");
    expect(dataScopeText("all")).toBe("全部");
  });

  it("returns '-' for undefined / null", () => {
    expect(dataScopeText(undefined)).toBe("-");
    expect(dataScopeText(null)).toBe("-");
  });

  it("falls back to String(v) for unknown values", () => {
    expect(dataScopeText("garbage")).toBe("garbage");
    expect(dataScopeText(99)).toBe("99");
  });
});

describe("dataScopeToNumber", () => {
  it("returns numeric value as-is", () => {
    expect(dataScopeToNumber(0)).toBe(0);
    expect(dataScopeToNumber(1)).toBe(1);
    expect(dataScopeToNumber(2)).toBe(2);
    expect(dataScopeToNumber(3)).toBe(3);
  });

  it("maps string enum to numeric code", () => {
    expect(dataScopeToNumber("self")).toBe(0);
    expect(dataScopeToNumber("dept")).toBe(1);
    expect(dataScopeToNumber("dept_and_below")).toBe(2);
    expect(dataScopeToNumber("all")).toBe(3);
  });

  it("treats empty/undefined/null as 0", () => {
    expect(dataScopeToNumber(undefined)).toBe(0);
    expect(dataScopeToNumber(null)).toBe(0);
    expect(dataScopeToNumber("")).toBe(0);
  });

  it("treats unknown string as 0", () => {
    expect(dataScopeToNumber("garbage")).toBe(0);
  });
});
