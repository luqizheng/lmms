// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { mount, type VueWrapper } from "@vue/test-utils";
import { h, nextTick } from "vue";
import { DropdownMenu } from "./index";

const items = [
  { label: "Edit", value: "edit" },
  { label: "Delete", value: "delete", danger: true },
  { label: "Archive", value: "archive", disabled: true }
];

async function flush() {
  await nextTick();
  // reka-ui 关闭浮层后用 setTimeout(0) 把焦点还给触发器，这里留出宏任务时间
  await new Promise(resolve => setTimeout(resolve, 10));
  await nextTick();
}

const wrappers: VueWrapper[] = [];

function mountMenu(props: Record<string, unknown> = {}) {
  const wrapper = mount(DropdownMenu, {
    props: { items, ariaLabel: "操作", ...props },
    slots: { default: "Actions" },
    attachTo: document.body
  });
  wrappers.push(wrapper);
  return wrapper;
}

function triggerOf(w: ReturnType<typeof mountMenu>) {
  return w.find<HTMLElement>('[aria-haspopup="menu"]').element;
}

function menuItemsInDom() {
  return Array.from(
    document.querySelectorAll<HTMLElement>('[role="menuitem"]')
  );
}

afterEach(() => {
  // 先卸载再清空 body：避免浮层还在时被外部移除导致 Vue 更新报错
  while (wrappers.length) wrappers.pop()?.unmount();
  document.body.innerHTML = "";
});

/** reka-ui 关闭浮层后还焦触发器是异步的，断言前等待其落地 */
async function waitForFocus(target: HTMLElement, timeout = 500) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (document.activeElement === target) return;
    await new Promise(resolve => setTimeout(resolve, 10));
  }
}

describe("DropdownMenu", () => {
  it("触发器带 aria-haspopup，初始收起", () => {
    const w = mountMenu();
    const trigger = w.find('[aria-haspopup="menu"]');
    expect(trigger.exists()).toBe(true);
    expect(trigger.attributes("aria-expanded")).toBe("false");
    expect(trigger.attributes("data-state")).toBe("closed");
  });

  it("打开后浮层为 menu，条目为 menuitem", async () => {
    const w = mountMenu();
    await w.find('[aria-haspopup="menu"]').trigger("click");
    await flush();
    expect(document.querySelector('[role="menu"]')).not.toBeNull();
    expect(menuItemsInDom().length).toBe(3);
  });

  it("点击菜单项触发 select 事件并关闭浮层、焦点还回触发器", async () => {
    const w = mountMenu();
    const trigger = triggerOf(w);
    await w.find('[aria-haspopup="menu"]').trigger("click");
    await flush();

    const edit = menuItemsInDom().find(node =>
      node.textContent?.includes("Edit")
    );
    edit?.click();
    await flush();

    const emitted = w.emitted("select");
    expect(emitted).toBeTruthy();
    expect(emitted?.[0]?.[0]).toMatchObject({ value: "edit", label: "Edit" });
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    await waitForFocus(trigger);
    expect(document.activeElement).toBe(trigger);
  });

  it("危险项带 danger 标记", async () => {
    const w = mountMenu();
    await w.find('[aria-haspopup="menu"]').trigger("click");
    await flush();
    const danger = menuItemsInDom().find(node =>
      node.textContent?.includes("Delete")
    );
    expect(danger?.className).toContain("ui-dropdown-menu__item--danger");
  });

  it("危险项可附带仅读屏可见的提示文案（颜色之外的第二重信号）", async () => {
    const w = mountMenu({
      items: [
        { label: "Edit", value: "edit" },
        {
          label: "Delete",
          value: "delete",
          danger: true,
          dangerLabel: "危险操作"
        }
      ]
    });
    await w.find('[aria-haspopup="menu"]').trigger("click");
    await flush();
    const danger = menuItemsInDom().find(node =>
      node.textContent?.includes("Delete")
    );
    const srOnly = danger?.querySelector(".ui-sr-only");
    expect(srOnly?.textContent).toContain("危险操作");
  });

  it("禁用项标记 aria-disabled 且不触发 select", async () => {
    const w = mountMenu();
    await w.find('[aria-haspopup="menu"]').trigger("click");
    await flush();
    const archive = menuItemsInDom().find(node =>
      node.textContent?.includes("Archive")
    );
    expect(archive?.getAttribute("aria-disabled")).toBe("true");
    expect(archive?.getAttribute("data-disabled")).toBeDefined();

    archive?.click();
    await flush();
    expect(w.emitted("select")).toBeUndefined();
  });

  it("键盘打开后首项预高亮，Enter 触发 select", async () => {
    const w = mountMenu();
    const trigger = triggerOf(w);
    trigger.focus();
    trigger.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true })
    );
    await flush();
    expect(trigger.getAttribute("aria-expanded")).toBe("true");
    expect(document.querySelector("[data-highlighted]")?.textContent).toContain(
      "Edit"
    );

    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Enter", bubbles: true })
    );
    await flush();
    expect(w.emitted("select")?.[0]?.[0]).toMatchObject({ value: "edit" });
  });

  it("Esc 关闭浮层并把焦点还给触发器", async () => {
    const w = mountMenu();
    const trigger = triggerOf(w);
    trigger.focus();
    trigger.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true })
    );
    await flush();
    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true })
    );
    await flush();
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    // 焦点归还同样是异步的：等它落到触发器上再断言
    await waitForFocus(trigger);
    expect(document.activeElement).toBe(trigger);
  });

  it("disabled 时触发器不可用", () => {
    const w = mountMenu({ disabled: true });
    expect(
      w.find('[aria-haspopup="menu"]').attributes("disabled")
    ).toBeDefined();
  });

  it("item 插槽可自定义菜单项内容", async () => {
    const w = mount(DropdownMenu, {
      props: { items, ariaLabel: "操作" },
      slots: {
        default: "Actions",
        item: (slotProps: { item: { label: string } }) =>
          h("span", { class: "custom" }, slotProps.item.label)
      },
      attachTo: document.body
    });
    wrappers.push(w);
    await w.find('[aria-haspopup="menu"]').trigger("click");
    await flush();
    expect(document.querySelector(".custom")?.textContent).toContain("Edit");
  });
});
