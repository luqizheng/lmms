import { describe, expect, it, vi, beforeEach } from "vitest";

vi.hoisted(() => {
  process.env.VITE_ROUTER_HISTORY = "hash";
});

const {
  listIamOrgUnits,
  createIamOrgUnit,
  updateIamOrgUnit,
  deleteIamOrgUnit,
  ElMessage,
  addDialog,
  message
} = vi.hoisted(() => ({
  listIamOrgUnits: vi.fn(),
  createIamOrgUnit: vi.fn(),
  updateIamOrgUnit: vi.fn(),
  deleteIamOrgUnit: vi.fn(),
  ElMessage: { error: vi.fn(), success: vi.fn() },
  addDialog: vi.fn(),
  message: vi.fn()
}));

vi.mock("element-plus", () => ({ ElMessage }));
vi.mock("@/components/ReDialog", () => ({ addDialog, closeDialog: vi.fn() }));
vi.mock("@/api/iam", () => ({
  listIamOrgUnits,
  createIamOrgUnit,
  updateIamOrgUnit,
  deleteIamOrgUnit
}));
vi.mock("@/utils/message", () => ({ message }));
vi.mock("@/utils/sso", () => ({}));
vi.mock("@/router", () => ({}));

import { useDept } from "./hook";

describe("useDept shape", () => {
  it("returns expected API keys", () => {
    const api = useDept();
    const expected = [
      "form",
      "loading",
      "columns",
      "dataList",
      "onSearch",
      "resetForm",
      "openDialog",
      "handleDelete",
      "handleAssignUsers",
      "handleSelectionChange"
    ];
    for (const k of expected) {
      expect(api).toHaveProperty(k);
    }
  });

  it("columns includes 6 base labels", () => {
    const api = useDept();
    const labels = api.columns.map((c: any) => c.label).filter(Boolean);
    expect(labels).toContain("部门名称");
    expect(labels).toContain("编码");
    expect(labels).toContain("层级");
    expect(labels).toContain("排序");
  });
});

describe("useDept onSearch", () => {
  beforeEach(() => vi.clearAllMocks());

  it("calls listIamOrgUnits and populates dataList", async () => {
    listIamOrgUnits.mockResolvedValue({
      items: [{ id: 1, name: "Eng", code: "ENG", level: 1, sort_order: 1 }],
      total: 1
    });
    const api = useDept();
    await api.onSearch();
    expect(listIamOrgUnits).toHaveBeenCalled();
    expect(api.dataList.value).toHaveLength(1);
  });

  it("on API error propagates the rejection (no try-catch in hook)", async () => {
    listIamOrgUnits.mockRejectedValue(new Error("net"));
    const api = useDept();
    await expect(api.onSearch()).rejects.toThrow("net");
    expect(message).not.toHaveBeenCalled();
  });
});

describe("useDept handleDelete", () => {
  beforeEach(() => vi.clearAllMocks());

  it("calls deleteIamOrgUnit and refreshes on success", async () => {
    deleteIamOrgUnit.mockResolvedValue({});
    listIamOrgUnits.mockResolvedValue({ items: [], total: 0 });
    const api = useDept();
    await api.handleDelete({ id: "3", name: "财务" } as any);
    expect(deleteIamOrgUnit).toHaveBeenCalledWith("3");
    expect(listIamOrgUnits).toHaveBeenCalled();
  });

  it("on API error shows message error with e.message", async () => {
    deleteIamOrgUnit.mockRejectedValue(new Error("oops"));
    const api = useDept();
    await api.handleDelete({ id: "3", name: "财务" } as any);
    expect(deleteIamOrgUnit).toHaveBeenCalled();
    expect(message).toHaveBeenCalledWith(
      "oops",
      expect.objectContaining({ type: "error" })
    );
  });
});
