import { describe, expect, it, vi, beforeEach } from "vitest";

const { mockStorage } = vi.hoisted(() => ({
  mockStorage: {
    config: vi.fn(),
    setItem: vi.fn(),
    getItem: vi.fn(),
    removeItem: vi.fn(),
    clear: vi.fn(),
    keys: vi.fn(),
    INDEXEDDB: "INDEXEDDB",
    LOCALSTORAGE: "LOCALSTORAGE"
  }
}));

vi.mock("localforage", () => ({ default: mockStorage }));

import { localForage } from "./index";

describe("StorageProxy", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockStorage.config.mockImplementation(() => undefined);
  });

  describe("setItem", () => {
    it("stores with expires=0 when m=0 (permanent)", async () => {
      mockStorage.setItem.mockResolvedValue({ data: "v", expires: 0 });
      const store = localForage();
      const out = await store.setItem("k", "v");
      expect(mockStorage.setItem).toHaveBeenCalledWith("k", {
        data: "v",
        expires: 0
      });
      expect(out).toBe("v");
    });

    it("stores with expires=now+m*60s when m>0", async () => {
      vi.useFakeTimers();
      vi.setSystemTime(new Date("2026-07-08T00:00:00Z"));
      mockStorage.setItem.mockResolvedValue({ data: "v", expires: 0 });
      const store = localForage();
      await store.setItem("k", "v", 5);
      expect(mockStorage.setItem).toHaveBeenCalledWith("k", {
        data: "v",
        expires: new Date("2026-07-08T00:00:00Z").getTime() + 5 * 60 * 1000
      });
      vi.useRealTimers();
    });
  });

  describe("getItem", () => {
    it("returns data when expires=0 (permanent)", async () => {
      mockStorage.getItem.mockResolvedValue({ data: "v", expires: 0 });
      const store = localForage();
      const out = await store.getItem("k");
      expect(out).toBe("v");
    });

    it("returns data when expires > now", async () => {
      vi.useFakeTimers();
      vi.setSystemTime(new Date("2026-07-08T00:00:00Z"));
      mockStorage.getItem.mockResolvedValue({
        data: "v",
        expires: new Date("2026-07-08T00:00:00Z").getTime() + 60_000
      });
      const store = localForage();
      const out = await store.getItem("k");
      expect(out).toBe("v");
      vi.useRealTimers();
    });

    it("returns null when expires < now (expired)", async () => {
      vi.useFakeTimers();
      vi.setSystemTime(new Date("2026-07-08T00:00:00Z"));
      mockStorage.getItem.mockResolvedValue({
        data: "v",
        expires: new Date("2026-07-07T00:00:00Z").getTime()
      });
      const store = localForage();
      const out = await store.getItem("k");
      expect(out).toBeNull();
      vi.useRealTimers();
    });
  });
});
