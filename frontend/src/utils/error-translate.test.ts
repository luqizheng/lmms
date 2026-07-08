import { describe, expect, it, beforeEach, afterEach } from "vitest";
import { translateError, setLocale, getLocale } from "./error-translate";

describe("translateError", () => {
  beforeEach(() => setLocale("zh-CN"));
  afterEach(() => setLocale("zh-CN"));

  it("returns zh-CN translation when present", () => {
    expect(translateError("IAM_AUTH_1001")).toBe("用户名或密码错误");
    expect(translateError("FILE_UPLOAD_2002")).toBe("文件大小超过限制");
  });

  it("falls back to English ErrorMessages when zh-CN misses", () => {
    setLocale("en");
    expect(translateError("IAM_AUTH_1001")).toBe("invalid credentials");
    expect(translateError("FILE_UPLOAD_2002")).toBe("file too large");
    setLocale("zh-CN");
  });

  it("falls back to code itself when neither map has it", () => {
    setLocale("en");
    expect(translateError("UNKNOWN_FAKE_9999")).toBe("UNKNOWN_FAKE_9999");
  });

  it("zh-CN overrides English for known codes", () => {
    setLocale("zh-CN");
    const zh = translateError("VALID_2003");
    expect(zh).toBe("手机号格式不正确");
  });
});

describe("setLocale / getLocale", () => {
  it("default is zh-CN", () => {
    expect(getLocale()).toBe("zh-CN");
  });

  it("switches locale", () => {
    setLocale("en");
    expect(getLocale()).toBe("en");
    setLocale("zh-CN");
    expect(getLocale()).toBe("zh-CN");
  });
});
