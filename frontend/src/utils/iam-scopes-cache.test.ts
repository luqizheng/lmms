import { describe, expect, it, vi, beforeEach } from "vitest";

const { listDepts, listRoles, listUsers } = vi.hoisted(() => ({
  listDepts: vi.fn(),
  listRoles: vi.fn(),
  listUsers: vi.fn()
}));

vi.mock("@/api/iam-scopes", () => ({
  listDepts,
  listRoles,
  listUsers,
  ScopeItem: {}
}));

import { loadScopes, invalidateScopes } from "./iam-scopes-cache";

describe("iam-scopes-cache", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    invalidateScopes();
    vi.useRealTimers();
  });

  it("fetches all 3 endpoints, normalizes, caches result", async () => {
    listDepts.mockResolvedValue([{ id: 1, name: "Engineering" }]);
    listRoles.mockResolvedValue([{ id: 2, code: "admin" }]);
    listUsers.mockResolvedValue({ list: [{ id: 3, username: "alice" }] });

    const r1 = await loadScopes();
    expect(r1.depts).toEqual([{ id: 1, name: "Engineering" }]);
    expect(r1.roles).toEqual([{ id: 2, name: "admin" }]);
    expect(r1.users).toEqual([{ id: 3, name: "alice" }]);
    expect(listDepts).toHaveBeenCalledTimes(1);
    expect(listRoles).toHaveBeenCalledTimes(1);
    expect(listUsers).toHaveBeenCalledTimes(1);

    const r2 = await loadScopes();
    expect(r2).toBe(r1);
    expect(listDepts).toHaveBeenCalledTimes(1);
  });

  it("expires after 5 minutes and refetches", async () => {
    listDepts.mockResolvedValue([{ id: 1, name: "Eng" }]);
    listRoles.mockResolvedValue([]);
    listUsers.mockResolvedValue({ list: [] });

    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-08T00:00:00Z"));
    await loadScopes();
    expect(listDepts).toHaveBeenCalledTimes(1);

    vi.setSystemTime(new Date("2026-07-08T00:06:00Z"));
    await loadScopes();
    expect(listDepts).toHaveBeenCalledTimes(2);
  });

  it("invalidateScopes() forces refetch on next call", async () => {
    listDepts.mockResolvedValue([{ id: 1, name: "Eng" }]);
    listRoles.mockResolvedValue([]);
    listUsers.mockResolvedValue({ list: [] });

    await loadScopes();
    expect(listDepts).toHaveBeenCalledTimes(1);
    invalidateScopes();
    await loadScopes();
    expect(listDepts).toHaveBeenCalledTimes(2);
  });
});
