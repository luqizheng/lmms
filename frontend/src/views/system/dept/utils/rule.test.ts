import { describe, expect, it } from "vitest";
import { formRules } from "./rule";

describe("dept-form-rule", () => {
  it("declares name and code", () => {
    const keys = Object.keys(formRules);
    expect(keys).toEqual(expect.arrayContaining(["name", "code"]));
    expect(keys).toHaveLength(2);
  });

  it("name and code are required with trigger=blur", () => {
    for (const k of ["name", "code"]) {
      const rule = (formRules as any)[k][0];
      expect(rule.required).toBe(true);
      expect(rule.trigger).toBe("blur");
      expect(typeof rule.message).toBe("string");
    }
  });
});
