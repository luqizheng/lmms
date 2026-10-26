// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mountShell } from "@/shell/test-utils";
import DashboardView from "./DashboardView.vue";

describe("概览", () => {
  it("渲染指标卡与标题", () => {
    const w = mountShell(DashboardView);
    expect(w.find(".ui-dashboard__title").exists()).toBe(true);
    expect(w.findAll(".ui-dashboard__metric").length).toBeGreaterThan(0);
  });

  it("指标值使用等宽数字", () => {
    const w = mountShell(DashboardView);
    expect(w.find(".ui-dashboard__metric-value").exists()).toBe(true);
  });
});
