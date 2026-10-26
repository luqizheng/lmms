// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Skeleton from "./Skeleton.vue";

describe("Skeleton", () => {
  it("对读屏隐藏", () => {
    expect(mount(Skeleton).attributes("aria-hidden")).toBe("true");
  });

  it("宽高可覆盖", () => {
    const w = mount(Skeleton, { props: { width: "120px", height: "20px" } });
    expect(w.attributes("style")).toContain("width: 120px");
    expect(w.attributes("style")).toContain("height: 20px");
  });

  it("缺省时占满宽度，高度随行高", () => {
    const style = mount(Skeleton).attributes("style") ?? "";
    expect(style).toContain("width: 100%");
    expect(style).toContain("height: 1em");
  });
});
