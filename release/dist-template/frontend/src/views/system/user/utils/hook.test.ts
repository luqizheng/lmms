import { describe, expect, it, vi, beforeEach } from "vitest";

vi.hoisted(() => {
  process.env.VITE_ROUTER_HISTORY = "hash";
});

const {
  listIamUsers,
  createIamUser,
  updateIamUser,
  deleteIamUser,
  listIamRoles,
  listIamOrgUnits,
  assignUserRoles,
  message,
  addDialog
} = vi.hoisted(() => ({
  listIamUsers: vi.fn(),
  createIamUser: vi.fn(),
  updateIamUser: vi.fn(),
  deleteIamUser: vi.fn(),
  listIamRoles: vi.fn(),
  listIamOrgUnits: vi.fn(),
  assignUserRoles: vi.fn(),
  message: vi.fn(),
  addDialog: vi.fn()
}));

vi.mock("element-plus", () => ({}));
vi.mock("@/components/ReDialog", () => ({ addDialog, closeDialog: vi.fn() }));
vi.mock("@/components/ReCropperPreview", () => ({ default: {} }));
vi.mock("@/api/iam", () => ({
  listIamUsers,
  createIamUser,
  updateIamUser,
  deleteIamUser,
  listIamRoles,
  listIamOrgUnits,
  assignUserRoles
}));
vi.mock("@/utils/message", () => ({ message }));
vi.mock("@/utils/sso", () => ({}));
vi.mock("@/router", () => ({}));
vi.mock("@/assets/user.jpg", () => ({ default: "mocked-avatar" }));
vi.mock("../../hooks", () => ({
  usePublicHooks: () => ({ switchStyle: { value: {} } })
}));

import { useUser } from "./hook";

function makeTableRef() {
  return {
    value: {
      getTableRef: () => ({
        getSelectionRows: vi.fn().mockReturnValue([]),
        clearSelection: vi.fn()
      }),
      setAdaptive: vi.fn()
    }
  };
}
function makeTreeRef() {
  return { value: {} };
}

describe("useUser shape", () => {
  it("returns expected API keys", () => {
    const api = useUser(makeTableRef() as any, makeTreeRef() as any);
    const expected = [
      "form",
      "loading",
      "columns",
      "dataList",
      "treeData",
      "treeLoading",
      "selectedNum",
      "pagination",
      "buttonClass",
      "deviceDetection",
      "onSearch",
      "resetForm",
      "onbatchDel",
      "openDialog",
      "onTreeSelect",
      "handleUpdate",
      "handleDelete",
      "handleUpload",
      "handleReset",
      "handleRole",
      "handleSizeChange",
      "onSelectionCancel",
      "handleCurrentChange",
      "handleSelectionChange"
    ];
    for (const k of expected) {
      expect(api).toHaveProperty(k);
    }
  });

  it("columns includes 10 base labels", () => {
    const api = useUser(makeTableRef() as any, makeTreeRef() as any);
    const labels = api.columns.map((c: any) => c.label).filter(Boolean);
    expect(labels).toContain("用户ID");
    expect(labels).toContain("用户名");
    expect(labels).toContain("邮箱");
    expect(labels).toContain("状态");
    expect(labels).toContain("最后登录");
  });
});

describe("useUser onSearch", () => {
  beforeEach(() => vi.clearAllMocks());

  it("calls listIamUsers with keyword+status+page+size and populates dataList", async () => {
    listIamUsers.mockResolvedValue({
      items: [{ id: 1, username: "alice", status: "active" }],
      total: 1,
      page: 1,
      size: 10
    });
    const api = useUser(makeTableRef() as any, makeTreeRef() as any);
    api.form.username = "al";
    api.form.status = "active";
    api.pagination.currentPage = 1;
    await api.onSearch();
    expect(listIamUsers).toHaveBeenCalledWith({
      keyword: "al",
      status: "active",
      page: 1,
      size: 10
    });
    expect(api.dataList.value).toHaveLength(1);
    expect(api.pagination.total).toBe(1);
  });

  it("on API error shows message error and sets loading false", async () => {
    listIamUsers.mockRejectedValue(new Error("net"));
    const api = useUser(makeTableRef() as any, makeTreeRef() as any);
    await api.onSearch();
    expect(message).toHaveBeenCalledWith(
      expect.stringContaining("加载用户列表失败"),
      expect.objectContaining({ type: "error" })
    );
    expect(api.loading.value).toBe(false);
  });
});

describe("useUser handleDelete", () => {
  beforeEach(() => vi.clearAllMocks());

  it("calls deleteIamUser and refreshes on success", async () => {
    deleteIamUser.mockResolvedValue({});
    listIamUsers.mockResolvedValue({ items: [], total: 0 });
    const api = useUser(makeTableRef() as any, makeTreeRef() as any);
    await api.handleDelete({ id: "5", username: "bob" } as any);
    expect(deleteIamUser).toHaveBeenCalledWith("5");
    expect(listIamUsers).toHaveBeenCalled();
  });

  it("on API error shows message error and does not refresh", async () => {
    deleteIamUser.mockRejectedValue(new Error("oops"));
    const api = useUser(makeTableRef() as any, makeTreeRef() as any);
    await api.handleDelete({ id: "5", username: "bob" } as any);
    expect(deleteIamUser).toHaveBeenCalled();
    expect(message).toHaveBeenCalledWith(
      expect.stringContaining("删除用户失败"),
      expect.objectContaining({ type: "error" })
    );
  });
});
