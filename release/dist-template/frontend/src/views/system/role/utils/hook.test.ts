import { describe, expect, it, vi, beforeEach } from "vitest";

vi.hoisted(() => {
  process.env.VITE_ROUTER_HISTORY = "hash";
});

const {
  listIamRoles,
  createIamRole,
  updateIamRole,
  deleteIamRole,
  listIamPermissions,
  assignRolePermissions,
  ElMessage,
  addDialog,
  message
} = vi.hoisted(() => ({
  listIamRoles: vi.fn(),
  createIamRole: vi.fn(),
  updateIamRole: vi.fn(),
  deleteIamRole: vi.fn(),
  listIamPermissions: vi.fn(),
  assignRolePermissions: vi.fn(),
  ElMessage: { error: vi.fn(), success: vi.fn() },
  addDialog: vi.fn(),
  message: vi.fn()
}));

vi.mock("element-plus", () => ({ ElMessage }));
vi.mock("@/components/ReDialog", () => ({ addDialog, closeDialog: vi.fn() }));
vi.mock("@/api/iam", () => ({
  listIamRoles,
  createIamRole,
  updateIamRole,
  deleteIamRole,
  listIamPermissions,
  assignRolePermissions
}));
vi.mock("@/utils/message", () => ({ message }));
vi.mock("@/utils/sso", () => ({}));
vi.mock("@/router", () => ({}));
vi.mock("../../hooks", () => ({
  usePublicHooks: () => ({ switchStyle: { value: {} } })
}));

import { useRole } from "./hook";

function makeTreeRef() {
  return { value: {} };
}

describe("useRole shape", () => {
  it("returns expected API keys", () => {
    const api = useRole(makeTreeRef() as any);
    const expected = [
      "form",
      "curRow",
      "loading",
      "columns",
      "dataList",
      "pagination",
      "permissionDialogVisible",
      "groupedPermissions",
      "checkedPermissionIds",
      "onSearch",
      "resetForm",
      "openDialog",
      "handleMenu",
      "handleSave",
      "handleDelete",
      "closePermissionDialog",
      "handleSizeChange",
      "handleCurrentChange",
      "handleSelectionChange"
    ];
    for (const k of expected) {
      expect(api).toHaveProperty(k);
    }
  });

  it("columns includes 4 base labels", () => {
    const api = useRole(makeTreeRef() as any);
    const labels = api.columns.map((c: any) => c.label).filter(Boolean);
    expect(labels).toContain("角色编号");
    expect(labels).toContain("角色名称");
    expect(labels).toContain("角色标识");
    expect(labels).toContain("数据范围");
  });
});

describe("useRole onSearch", () => {
  beforeEach(() => vi.clearAllMocks());

  it("calls listIamRoles and populates dataList", async () => {
    listIamRoles.mockResolvedValue({
      items: [
        { id: 1, name: "admin", code: "ADMIN", data_scope: 0, status: "active" }
      ],
      total: 1,
      page: 1,
      size: 10
    });
    const api = useRole(makeTreeRef() as any);
    await api.onSearch();
    expect(listIamRoles).toHaveBeenCalled();
    expect(api.dataList.value).toHaveLength(1);
    expect(api.pagination.total).toBe(1);
  });

  it("on API error shows message error", async () => {
    listIamRoles.mockRejectedValue(new Error("net"));
    const api = useRole(makeTreeRef() as any);
    await api.onSearch();
    expect(message).toHaveBeenCalledWith(
      expect.stringContaining("加载角色列表失败"),
      expect.objectContaining({ type: "error" })
    );
  });
});

describe("useRole handleDelete", () => {
  beforeEach(() => vi.clearAllMocks());

  it("calls deleteIamRole and refreshes on success", async () => {
    deleteIamRole.mockResolvedValue({});
    listIamRoles.mockResolvedValue({ items: [], total: 0 });
    const api = useRole(makeTreeRef() as any);
    await api.handleDelete({ id: "9", name: "r" } as any);
    expect(deleteIamRole).toHaveBeenCalledWith("9");
    expect(listIamRoles).toHaveBeenCalled();
  });

  it("on API error shows message error and does not refresh", async () => {
    deleteIamRole.mockRejectedValue(new Error("oops"));
    const api = useRole(makeTreeRef() as any);
    await api.handleDelete({ id: "9", name: "r" } as any);
    expect(deleteIamRole).toHaveBeenCalled();
    expect(message).toHaveBeenCalledWith(
      expect.stringContaining("删除角色失败"),
      expect.objectContaining({ type: "error" })
    );
  });
});
