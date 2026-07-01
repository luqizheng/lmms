import { describe, expect, it, beforeEach, vi } from "vitest";
import { useInboxReadState } from "./read-state";

// vi.stubGlobal 模拟 localStorage（vitest happy-dom 默认可用，但显式 stub 让意图清楚）
const memStore = new Map<string, string>();
beforeEach(() => {
  memStore.clear();
  vi.stubGlobal("localStorage", {
    getItem: (k: string) => (memStore.has(k) ? memStore.get(k)! : null),
    setItem: (k: string, v: string) => void memStore.set(k, v),
    removeItem: (k: string) => void memStore.delete(k),
    clear: () => memStore.clear()
  });
});

describe("useInboxReadState", () => {
  it("starts empty", () => {
    const s = useInboxReadState();
    expect(s.isRead(1)).toBe(false);
    expect(s.isRead(2)).toBe(false);
  });

  it("marks ids as read", () => {
    const s = useInboxReadState();
    s.markRead(7);
    expect(s.isRead(7)).toBe(true);
    expect(s.isRead(8)).toBe(false);
  });

  it("persists to localStorage", () => {
    const s = useInboxReadState();
    s.markRead(42);
    // 触发 ref watch flush（vue 会同步推）
    return Promise.resolve().then(() => {
      const raw = localStorage.getItem("notification_inbox_read_ids");
      expect(raw).not.toBeNull();
      expect(JSON.parse(raw!)).toContain(42);
    });
  });

  it("unreadIds filters out read ids", () => {
    const s = useInboxReadState();
    s.markRead(2);
    expect(s.unreadIds([1, 2, 3])).toEqual([1, 3]);
  });

  it("idempotent markRead", () => {
    const s = useInboxReadState();
    s.markRead(5);
    s.markRead(5);
    expect(s.isRead(5)).toBe(true);
  });
});
