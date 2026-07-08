import { describe, expect, it } from "vitest";
import { STATUS_LABEL_MAP, getStatusInfo } from "./status";

describe("STATUS_LABEL_MAP", () => {
  it("has 3 known statuses", () => {
    expect(Object.keys(STATUS_LABEL_MAP).sort()).toEqual([
      "draft",
      "published",
      "revoked"
    ]);
  });

  it("draft / published / revoked labels and types", () => {
    expect(STATUS_LABEL_MAP.draft).toEqual({ label: "草稿", type: "info" });
    expect(STATUS_LABEL_MAP.published).toEqual({
      label: "已发布",
      type: "success"
    });
    expect(STATUS_LABEL_MAP.revoked).toEqual({
      label: "已撤销",
      type: "danger"
    });
  });
});

describe("getStatusInfo", () => {
  it("returns mapped entry for known status", () => {
    expect(getStatusInfo("draft")).toEqual({ label: "草稿", type: "info" });
    expect(getStatusInfo("published")).toEqual({
      label: "已发布",
      type: "success"
    });
  });

  it("falls back to {label, info} for unknown status", () => {
    expect(getStatusInfo("garbage")).toEqual({
      label: "garbage",
      type: "info"
    });
    expect(getStatusInfo("")).toEqual({ label: "", type: "info" });
  });
});
