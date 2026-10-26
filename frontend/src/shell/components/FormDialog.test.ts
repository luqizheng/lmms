// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "../test-utils";
import FormDialog from "./FormDialog.vue";

const wrappers: Array<{ unmount: () => void }> = [];

afterEach(() => {
  // 弹框内容 teleport 到 body，必须先卸载再清空，否则会拿到上一个用例的 DOM
  while (wrappers.length) wrappers.pop()?.unmount();
  document.body.innerHTML = "";
});

function mountDialog(props: Record<string, unknown> = {}) {
  const wrapper = mountShell(FormDialog, {
    props: { open: true, title: "编辑用户", ...props }
  });
  wrappers.push(wrapper);
  return wrapper;
}

describe("FormDialog", () => {
  it("打开时渲染标题与表单", async () => {
    const w = mountShell(FormDialog, {
      props: { open: true, title: "编辑用户" },
      slots: { default: "<input class='mock-field' />" }
    });
    wrappers.push(w);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    const dialog = document.querySelector<HTMLElement>('[role="dialog"]');
    expect(dialog?.textContent).toContain("编辑用户");
    expect(dialog?.querySelector(".mock-field")).not.toBeNull();
  });

  it("提交表单时回传 submit", async () => {
    const w = mountDialog();
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    const form = document.querySelector("form");
    form?.dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true })
    );
    await nextTick();
    expect(w.emitted("submit")).toBeTruthy();
  });

  it("取消时回传 cancel", async () => {
    const w = mountDialog();
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    const cancel = document.querySelector<HTMLElement>(
      ".ui-form-dialog__footer button"
    );
    cancel?.click();
    await nextTick();
    expect(w.emitted("cancel")).toBeTruthy();
  });
});
