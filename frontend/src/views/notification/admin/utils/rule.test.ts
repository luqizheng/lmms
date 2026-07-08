import { describe, expect, it } from "vitest";
import { formRules } from "./rule";

describe("notification-admin-form-rule", () => {
  it("declares title / body / publish_at", () => {
    const keys = Object.keys(formRules);
    expect(keys).toEqual(expect.arrayContaining(["title", "body", "publish_at"]));
    expect(keys).toHaveLength(3);
  });

  it("title and body are required with trigger=blur", () => {
    for (const k of ["title", "body"]) {
      const rule = (formRules as any)[k][0];
      expect(rule.required).toBe(true);
      expect(rule.trigger).toBe("blur");
    }
  });

  it("publish_at is required with trigger=change (date picker needs change)", () => {
    const rule = (formRules as any).publish_at[0];
    expect(rule.required).toBe(true);
    expect(rule.trigger).toBe("change");
  });
});
