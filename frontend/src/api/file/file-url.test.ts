// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { fileUrl } from "./file-url";

describe("fileUrl", () => {
  it("returns '' for nullish / empty / whitespace input", () => {
    expect(fileUrl(undefined)).toBe("");
    expect(fileUrl(null)).toBe("");
    expect(fileUrl("")).toBe("");
    expect(fileUrl("   ")).toBe("");
  });

  it("passes absolute http(s) URLs through unchanged", () => {
    expect(fileUrl("http://example.com/v1/file/abc")).toBe(
      "http://example.com/v1/file/abc"
    );
    expect(fileUrl("https://example.com/v1/file/abc")).toBe(
      "https://example.com/v1/file/abc"
    );
    expect(fileUrl("  https://example.com/x  ")).toBe("https://example.com/x");
  });

  it("prefixes leading-slash relative paths with the VITE_API_BASE fallback", () => {
    // happy-dom 下 import.meta.env.VITE_API_BASE 解析不出来,
    // 走源码里 ?? "/api" 的 fallback 分支
    expect(fileUrl("/v1/file/abc")).toBe("/api/v1/file/abc");
  });

  it("prefixes bare relative paths without a leading slash", () => {
    expect(fileUrl("v1/file/abc")).toBe("/api/v1/file/abc");
  });
});
