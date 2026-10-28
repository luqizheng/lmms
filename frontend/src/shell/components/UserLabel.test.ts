// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia } from "pinia";
import { i18n } from "../i18n";
import UserLabel from "./UserLabel.vue";

const { listUsersSimple } = vi.hoisted(() => ({ listUsersSimple: vi.fn() }));

vi.mock("../api/users", () => ({ listUsersSimple }));

function page(
  items: { id: string; nickname: string }[],
  total: number,
  pageNo = 1
) {
  return { items, total, page: pageNo, size: 200 };
}

function mountLabel(props: { userId?: string | null }, pinia = createPinia()) {
  return mount(UserLabel, { props, global: { plugins: [pinia, i18n] } });
}

describe("UserLabel", () => {
  beforeEach(() => {
    listUsersSimple.mockReset();
  });

  it("按 userId 解析出昵称", async () => {
    listUsersSimple.mockResolvedValue(
      page([{ id: "dev__1", nickname: "张三" }], 1)
    );
    const w = mountLabel({ userId: "dev__1" });
    await flushPromises();
    expect(w.text()).toBe("张三");
  });

  it("同一 userId 只向服务器请求一次", async () => {
    listUsersSimple.mockResolvedValue(
      page([{ id: "dev__1", nickname: "张三" }], 1)
    );
    const pinia = createPinia();
    const a = mountLabel({ userId: "dev__1" }, pinia);
    const b = mountLabel({ userId: "dev__1" }, pinia);
    await flushPromises();
    expect(a.text()).toBe("张三");
    expect(b.text()).toBe("张三");
    expect(listUsersSimple).toHaveBeenCalledTimes(1);
  });

  it("回退文案剥离 dev__ 环境前缀", async () => {
    listUsersSimple.mockResolvedValue(page([], 0));
    const w = mountLabel({ userId: "dev__404" });
    await flushPromises();
    expect(w.text()).toBe("用户 #404");
  });

  it("回退文案保留无前缀 ID 原样", async () => {
    listUsersSimple.mockResolvedValue(page([], 0));
    const w = mountLabel({ userId: "ABC123" });
    await flushPromises();
    expect(w.text()).toBe("用户 #ABC123");
  });

  it("无 userId 时显示占位且不请求", async () => {
    listUsersSimple.mockResolvedValue(page([], 0));
    const w = mountLabel({ userId: null });
    await flushPromises();
    expect(w.text()).toBe("—");
    expect(listUsersSimple).not.toHaveBeenCalled();
  });

  it("目录分页时合并所有页", async () => {
    listUsersSimple
      .mockResolvedValueOnce(page([{ id: "u1", nickname: "甲" }], 201, 1))
      .mockResolvedValueOnce(page([{ id: "u2", nickname: "乙" }], 201, 2));
    const w = mountLabel({ userId: "u2" });
    await flushPromises();
    await flushPromises();
    expect(w.text()).toBe("乙");
    expect(listUsersSimple).toHaveBeenCalledTimes(2);
  });
});
