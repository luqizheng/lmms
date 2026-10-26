import { describe, expect, it } from "vitest";
import { cn } from "./cn";

describe("cn", () => {
  it("拼接多个 class", () => {
    expect(cn("a", "b")).toBe("a b");
  });

  it("忽略 falsy 值", () => {
    expect(cn("a", false, undefined, null, "", "b")).toBe("a b");
  });

  it("支持数组与对象条件", () => {
    expect(cn(["a", "b"], { c: true, d: false })).toBe("a b c");
  });

  it("后写的冲突 Tailwind 类覆盖先写的", () => {
    expect(cn("px-2", "px-4")).toBe("px-4");
  });
});
