// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { Toast, ToastHost } from "./index";

const toastSource = readFileSync(resolve(__dirname, "Toast.vue"), "utf8");
const hostSource = readFileSync(resolve(__dirname, "ToastHost.vue"), "utf8");

const wrappers: Array<{ unmount: () => void }> = [];

function mountToast(props: Record<string, unknown> = {}) {
  const wrapper = mount(Toast, {
    props: { title: "标题", ...props },
    attachTo: document.body
  });
  wrappers.push(wrapper);
  return wrapper;
}

async function flush() {
  await nextTick();
  await new Promise(resolve => setTimeout(resolve, 0));
  await nextTick();
}

function toastEl(): HTMLElement | null {
  return document.querySelector<HTMLElement>(".ui-toast");
}

afterEach(() => {
  while (wrappers.length) wrappers.pop()?.unmount();
  document.body.innerHTML = "";
  vi.useRealTimers();
});

describe("Toast", () => {
  it("success / info 使用 role=status（不打断当前朗读）", () => {
    mountToast({ variant: "success" });
    expect(toastEl()?.getAttribute("role")).toBe("status");
    document.body.innerHTML = "";

    mountToast({ variant: "info" });
    expect(toastEl()?.getAttribute("role")).toBe("status");
  });

  it("warning / danger 使用 role=alert（立即播报）", () => {
    mountToast({ variant: "warning" });
    expect(toastEl()?.getAttribute("role")).toBe("alert");
    document.body.innerHTML = "";

    mountToast({ variant: "danger" });
    expect(toastEl()?.getAttribute("role")).toBe("alert");
  });

  it("渲染标题与描述", () => {
    mountToast({
      variant: "success",
      title: "保存成功",
      description: "配置已生效"
    });
    expect(toastEl()?.textContent).toContain("保存成功");
    expect(toastEl()?.textContent).toContain("配置已生效");
  });

  it("成功 toast 必须可关：右上角 × 按钮存在且带无障碍名称", () => {
    mountToast({ variant: "success", closeLabel: "关闭" });
    const close = document.querySelector<HTMLElement>(".ui-toast__close");
    expect(close).not.toBeNull();
    expect(close?.tagName).toBe("BUTTON");
    expect(close?.getAttribute("aria-label")).toBe("关闭");
  });

  it("点击关闭按钮请求关闭", async () => {
    const wrapper = mountToast({ variant: "success" });
    await flush();
    const close = document.querySelector<HTMLElement>(".ui-toast__close");
    close?.click();
    await nextTick();
    expect(wrapper.emitted("update:open")).toEqual([[false]]);
  });

  it("closable=false 时不渲染关闭按钮", () => {
    mountToast({ variant: "danger", closable: false });
    expect(document.querySelector(".ui-toast__close")).toBeNull();
  });

  it("不抢焦点：弹出后焦点仍在原处", async () => {
    const outside = document.createElement("button");
    document.body.appendChild(outside);
    outside.focus();
    expect(document.activeElement).toBe(outside);

    mountToast({ variant: "success" });
    await flush();
    expect(document.activeElement).toBe(outside);
    outside.remove();
  });

  it("停留 4000ms 后自动关闭", async () => {
    vi.useFakeTimers();
    const wrapper = mountToast({ variant: "success", duration: 4000 });
    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(0);

    await vi.advanceTimersByTimeAsync(3999);
    expect(wrapper.emitted("update:open")).toBeUndefined();

    await vi.advanceTimersByTimeAsync(1);
    expect(wrapper.emitted("update:open")).toEqual([[false]]);
  });

  it("悬停暂停计时，暂停期间不会自动关闭", async () => {
    vi.useFakeTimers();
    const wrapper = mountToast({ variant: "success", duration: 4000 });
    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(0);

    const viewport = document.querySelector<HTMLElement>(".ui-toast__viewport");
    expect(viewport).not.toBeNull();
    viewport?.dispatchEvent(new Event("pointermove", { bubbles: true }));
    await vi.advanceTimersByTimeAsync(0);

    await vi.advanceTimersByTimeAsync(10000);
    expect(wrapper.emitted("update:open")).toBeUndefined();
  });

  it("白卡 + shadow-md + border-strong + 左侧 3px 语义色竖条，全部来自 token", () => {
    expect(toastSource).toContain("var(--surface-raised)");
    expect(toastSource).toContain("var(--shadow-md)");
    expect(toastSource).toContain("var(--border-strong)");
    expect(toastSource).toContain("var(--success)");
    expect(toastSource).toContain("var(--danger)");
    expect(toastSource).toContain("var(--warning)");
    expect(toastSource).toContain("var(--info)");
    // 3px 竖条 = --border-2(2px) + --hairline(1px)
    expect(toastSource).toContain("calc(var(--border-2) + var(--hairline))");
    // 关闭按钮 24×24 = --space-6
    expect(toastSource).toContain("var(--space-6)");
    // 进入 / 关闭动画 240ms = --duration-medium
    expect(toastSource).toContain("var(--duration-medium)");
    expect(toastSource).not.toMatch(/\b240ms\b/);
  });

  it("ToastHost 同屏最多 1 条，新 toast 替换旧 toast", async () => {
    const host = mount(ToastHost, { attachTo: document.body });
    wrappers.push(host);

    host.vm.toast({ variant: "success", title: "第一条" });
    await flush();
    expect(document.querySelectorAll(".ui-toast")).toHaveLength(1);
    expect(toastEl()?.textContent).toContain("第一条");

    host.vm.toast({ variant: "danger", title: "第二条" });
    await flush();
    expect(document.querySelectorAll(".ui-toast")).toHaveLength(1);
    expect(toastEl()?.textContent).toContain("第二条");
    expect(toastEl()?.textContent).not.toContain("第一条");
  });

  it("ToastHost 暴露 toast / dismiss 接口", async () => {
    const host = mount(ToastHost, { attachTo: document.body });
    wrappers.push(host);

    host.vm.toast({ variant: "info", title: "占位提示" });
    await flush();
    expect(toastEl()?.getAttribute("role")).toBe("status");

    host.vm.dismiss();
    await flush();
    expect(document.querySelectorAll(".ui-toast")).toHaveLength(0);
  });

  it("ToastHost 自身不引入裸色值与未定义 token", () => {
    expect(hostSource).not.toMatch(/#[0-9a-fA-F]{3,8}/);
    expect(hostSource).not.toMatch(/rgba?\(/);
  });
});
