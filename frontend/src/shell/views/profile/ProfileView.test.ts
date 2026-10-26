// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { mountShell } from "@/shell/test-utils";
import ProfileView from "./ProfileView.vue";

vi.mock("@/shell/store/auth", () => ({
  useAuthStore: () => ({
    user: {
      id: "1",
      username: "admin",
      nickname: "管理员",
      email: "admin@example.com",
      roles: ["admin"]
    }
  })
}));

describe("个人设置", () => {
  it("渲染当前用户的昵称、邮箱与角色", () => {
    const w = mountShell(ProfileView);
    expect(w.text()).toContain("管理员");
    expect(w.text()).toContain("admin@example.com");
    expect(w.text()).toContain("admin");
  });
});
