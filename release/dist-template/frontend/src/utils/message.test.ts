import { describe, expect, it, vi, beforeEach } from "vitest";

vi.hoisted(() => {
  Object.defineProperty(globalThis, "document", {
    value: {
      body: {},
      createElement: () => ({
        style: {},
        setAttribute: () => undefined,
        appendChild: () => undefined,
        insertBefore: () => undefined,
        removeChild: () => undefined,
        addEventListener: () => undefined
      })
    },
    configurable: true
  });
});

const { elMessage, closeAll } = vi.hoisted(() => {
  const elMessageFn: any = vi.fn();
  elMessageFn.closeAll = vi.fn();
  return { elMessage: elMessageFn, closeAll: elMessageFn.closeAll };
});

vi.mock("@pureadmin/utils", () => ({
  isFunction: (v: unknown) => typeof v === "function"
}));

vi.mock("element-plus", () => ({
  ElMessage: elMessage,
  closeAll
}));

import { message, closeAllMessage } from "./message";

describe("message()", () => {
  beforeEach(() => {
    elMessage.mockReset();
  });

  it("no-params path uses 'pure-message' customClass", () => {
    message("hello");
    expect(elMessage).toHaveBeenCalledWith({
      message: "hello",
      customClass: "pure-message"
    });
  });

  it("with no params defaults customClass to 'pure-message'", () => {
    message("hello");
    expect(elMessage).toHaveBeenCalledWith({
      message: "hello",
      customClass: "pure-message"
    });
  });

  it("with partial params applies documented defaults", () => {
    message("hi", { type: "warning" });
    const arg = elMessage.mock.calls[0][0];
    expect(arg.type).toBe("warning");
    expect(arg.duration).toBe(2000);
    expect(arg.offset).toBe(16);
    expect(arg.placement).toBe("top");
    expect(arg.grouping).toBe(false);
    expect(arg.repeatNum).toBe(1);
    expect(arg.showClose).toBe(false);
  });

  it("'antd' customClass maps to 'pure-message', 'el' maps to ''", () => {
    message("a", { customClass: "antd" });
    expect(elMessage.mock.calls[0][0].customClass).toBe("pure-message");
    message("b", { customClass: "el" });
    expect(elMessage.mock.calls[1][0].customClass).toBe("");
  });

  it("all-explicit params pass through verbatim", () => {
    message("x", {
      type: "error",
      duration: 5000,
      offset: 32,
      placement: "bottom-right",
      showClose: true,
      grouping: true,
      repeatNum: 3
    });
    const arg = elMessage.mock.calls[0][0];
    expect(arg.type).toBe("error");
    expect(arg.duration).toBe(5000);
    expect(arg.offset).toBe(32);
    expect(arg.placement).toBe("bottom-right");
    expect(arg.showClose).toBe(true);
    expect(arg.grouping).toBe(true);
    expect(arg.repeatNum).toBe(3);
  });

  it("onClose is invoked when ElMessage's onClose callback fires", () => {
    const userCb = vi.fn();
    message("y", { onClose: userCb });
    const arg = elMessage.mock.calls[0][0];
    arg.onClose();
    expect(userCb).toHaveBeenCalledTimes(1);
  });
});

describe("closeAllMessage()", () => {
  it("delegates to ElMessage.closeAll", () => {
    closeAllMessage();
    expect(closeAll).toHaveBeenCalledTimes(1);
  });
});
