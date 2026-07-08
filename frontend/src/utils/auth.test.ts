import { describe, expect, it, vi, beforeEach } from "vitest";

vi.mock("@/store/modules/user", () => ({
  useUserStoreHook: vi.fn()
}));
vi.mock("@pureadmin/utils", () => ({
  isString: (v: unknown) => typeof v === "string",
  isIncludeAllChildren: (
    parents: string[] | string,
    children: string[]
  ): boolean => {
    if (typeof parents === "string") return children.includes(parents);
    return parents.every(p => children.includes(p));
  }
}));

import { useUserStoreHook } from "@/store/modules/user";
import {
  formatToken,
  hasPerms,
  userKey,
  TokenKey,
  multipleTabsKey
} from "./auth";

const mockStore = (permissions: string[] | undefined) => {
  (useUserStoreHook as any).mockReturnValue({ permissions });
};

describe("constants", () => {
  it("exposes canonical localStorage / cookie keys", () => {
    expect(userKey).toBe("user-info");
    expect(TokenKey).toBe("authorized-token");
    expect(multipleTabsKey).toBe("multiple-tabs");
  });
});

describe("formatToken", () => {
  it("prefixes with 'Bearer '", () => {
    expect(formatToken("abc.def.ghi")).toBe("Bearer abc.def.ghi");
    expect(formatToken("")).toBe("Bearer ");
  });
});

describe("hasPerms", () => {
  beforeEach(() => {
    (useUserStoreHook as any).mockReset();
  });

  it("returns false when value is empty / null", () => {
    mockStore(["IAM_USER_READ"]);
    expect(hasPerms("")).toBe(false);
    expect(hasPerms(null as any)).toBe(false);
    expect(hasPerms(undefined as any)).toBe(false);
  });

  it("returns false when permissions missing", () => {
    mockStore(undefined);
    expect(hasPerms("IAM_USER_READ")).toBe(false);
  });

  it("returns true when user has *:*:* wildcard", () => {
    mockStore(["*:*:*"]);
    expect(hasPerms("IAM_USER_DELETE")).toBe(true);
  });

  it("returns true when value string is in permissions", () => {
    mockStore(["IAM_USER_READ", "IAM_USER_UPDATE"]);
    expect(hasPerms("IAM_USER_READ")).toBe(true);
    expect(hasPerms("IAM_USER_DELETE")).toBe(false);
  });

  it("returns true only when ALL of array value are in permissions", () => {
    mockStore(["A", "B"]);
    expect(hasPerms(["A", "B"])).toBe(true);
    expect(hasPerms(["A", "C"])).toBe(false);
  });
});
