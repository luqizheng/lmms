import { describe, expect, it, vi } from "vitest";
import { formRules } from "./rule";

describe("user-form-rule", () => {
  it("declares 5 expected fields", () => {
    const keys = Object.keys(formRules);
    expect(keys).toEqual(
      expect.arrayContaining([
        "nickname",
        "username",
        "password",
        "phone",
        "email"
      ])
    );
    expect(keys).toHaveLength(5);
  });

  it("nickname / username / password are required with trigger=blur", () => {
    for (const k of ["nickname", "username", "password"]) {
      const rule = (formRules as any)[k][0];
      expect(rule.required).toBe(true);
      expect(rule.trigger).toBe("blur");
    }
  });

  it("phone validator: empty value passes (no error)", () => {
    const cb = vi.fn();
    (formRules as any).phone[0].validator({}, "", cb);
    expect(cb).toHaveBeenCalledTimes(1);
    expect(cb.mock.calls[0]).toHaveLength(0);
  });

  it("phone validator: invalid value rejects with error", () => {
    const cb = vi.fn();
    (formRules as any).phone[0].validator({}, "not-a-phone", cb);
    expect(cb).toHaveBeenCalledTimes(1);
    const arg = cb.mock.calls[0][0];
    expect(arg).toBeInstanceOf(Error);
    expect(arg.message).toBe("请输入正确的手机号码格式");
  });

  it("phone validator: valid Chinese mobile passes", () => {
    const cb = vi.fn();
    (formRules as any).phone[0].validator({}, "13800138000", cb);
    expect(cb).toHaveBeenCalledTimes(1);
    expect(cb.mock.calls[0]).toHaveLength(0);
  });

  it("email validator: invalid value rejects with error", () => {
    const cb = vi.fn();
    (formRules as any).email[0].validator({}, "not-an-email", cb);
    const arg = cb.mock.calls[0][0];
    expect(arg).toBeInstanceOf(Error);
    expect(arg.message).toBe("请输入正确的邮箱格式");
  });

  it("email validator: valid email passes", () => {
    const cb = vi.fn();
    (formRules as any).email[0].validator({}, "user@example.com", cb);
    expect(cb.mock.calls[0]).toHaveLength(0);
  });
});
