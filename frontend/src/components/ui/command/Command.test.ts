// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { Command } from "./index";

const items = [
  { id: "deploy", label: "新建部署", group: "部署" },
  { id: "logs", label: "查看日志", group: "部署" },
  { id: "users", label: "成员管理", group: "设置" }
];

const wrappers: Array<{ unmount: () => void }> = [];

function mountCommand(props: Record<string, unknown> = {}) {
  const wrapper = mount(Command, {
    props: { open: true, items, ...props },
    attachTo: document.body
  });
  wrappers.push(wrapper);
  return wrapper;
}

afterEach(() => {
  while (wrappers.length) wrappers.pop()?.unmount();
  document.body.innerHTML = "";
});

describe("Command", () => {
  it("渲染为命令面板：dialog + combobox 输入框", () => {
    mountCommand();
    expect(document.querySelector('[role="dialog"]')).not.toBeNull();
    const input = document.querySelector<HTMLInputElement>('[role="combobox"]');
    expect(input).not.toBeNull();
    expect(input?.getAttribute("aria-expanded")).toBe("true");
  });

  it("列表项全部渲染且带 role=option", () => {
    mountCommand();
    const options = document.querySelectorAll('[role="option"]');
    expect(options.length).toBe(3);
    expect(options[0].textContent).toContain("新建部署");
  });

  it("输入过滤列表", async () => {
    mountCommand();
    const input =
      document.querySelector<HTMLInputElement>('[role="combobox"]')!;
    input.value = "成员";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
    const options = document.querySelectorAll('[role="option"]');
    expect(options.length).toBe(1);
    expect(options[0].textContent).toContain("成员管理");
  });

  it("无结果时给出空态提示", async () => {
    mountCommand({ emptyText: "未找到匹配结果" });
    const input =
      document.querySelector<HTMLInputElement>('[role="combobox"]')!;
    input.value = "zzz";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
    expect(document.body.textContent).toContain("未找到匹配结果");
  });

  it("上下键移动高亮，Enter 选中并回传 id", async () => {
    const w = mountCommand();
    const input =
      document.querySelector<HTMLInputElement>('[role="combobox"]')!;

    // 初始高亮第一项
    expect(
      document
        .querySelectorAll('[role="option"]')[0]
        .getAttribute("aria-selected")
    ).toBe("true");

    input.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true })
    );
    await nextTick();
    expect(
      document
        .querySelectorAll('[role="option"]')[1]
        .getAttribute("aria-selected")
    ).toBe("true");

    input.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Enter", bubbles: true })
    );
    await nextTick();
    expect(w.emitted("select")?.[0]).toEqual(["logs"]);
  });

  it("Esc 请求关闭", async () => {
    const w = mountCommand();
    const input =
      document.querySelector<HTMLInputElement>('[role="combobox"]')!;
    input.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true })
    );
    await nextTick();
    expect(w.emitted("update:open")).toEqual([[false]]);
  });
});
