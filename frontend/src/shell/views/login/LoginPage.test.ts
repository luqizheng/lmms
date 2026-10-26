// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import LoginPage from "./LoginPage.vue";
import { mountShell } from "../../test-utils";

vi.mock("vue-router", () => ({
  useRoute: () => ({ query: {} }),
  useRouter: () => ({ replace: vi.fn().mockResolvedValue(undefined) })
}));

vi.mock("../../api/client", () => ({ onUnauthorized: vi.fn() }));

const signIn = vi.fn();
vi.mock("../../store/auth", () => ({
  useAuthStore: () => ({ signIn, reset: vi.fn() })
}));

describe("LoginPage", () => {
  it("渲染品牌名与登录表单", () => {
    const w = mountShell(LoginPage);
    expect(w.text()).toContain("PRISM 控制台");
    expect(w.find("input[type='text']").exists()).toBe(true);
    expect(w.find("input[type='password']").exists()).toBe(true);
    expect(w.find("button[type='submit']").exists()).toBe(true);
  });

  it("用户名或密码为空时不发请求并给出提示", async () => {
    const w = mountShell(LoginPage);
    await w.find("form").trigger("submit");
    await nextTick();
    expect(signIn).not.toHaveBeenCalled();
    expect(w.find(".ui-login__error").text()).toBe("请输入用户名与密码");
  });

  it("登录失败时展示错误信息", async () => {
    signIn.mockRejectedValueOnce({ code: "IAM_AUTH_1002" });
    const w = mountShell(LoginPage);
    await w.find("input[type='text']").setValue("admin");
    await w.find("input[type='password']").setValue("wrong");
    await w.find("form").trigger("submit");
    await nextTick();
    await nextTick();
    expect(w.find(".ui-login__error").exists()).toBe(true);
  });
});
