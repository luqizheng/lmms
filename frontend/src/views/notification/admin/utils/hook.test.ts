import { describe, expect, it, vi, beforeEach } from "vitest";

vi.hoisted(() => {
  process.env.VITE_ROUTER_HISTORY = "hash";
});

const {
  listAnnouncements,
  createAnnouncement,
  publishAnnouncement,
  revokeAnnouncement,
  ElMessage,
  ElMessageBox,
  addDialog,
  useDebounceFn
} = vi.hoisted(() => ({
  listAnnouncements: vi.fn(),
  createAnnouncement: vi.fn(),
  publishAnnouncement: vi.fn(),
  revokeAnnouncement: vi.fn(),
  ElMessage: { error: vi.fn(), success: vi.fn() },
  ElMessageBox: { confirm: vi.fn().mockResolvedValue("confirm") },
  addDialog: vi.fn(),
  useDebounceFn: (fn: any) => fn
}));

vi.mock("element-plus", () => ({
  ElMessage,
  ElMessageBox
}));
vi.mock("@vueuse/core", () => ({ useDebounceFn }));
vi.mock("@/components/ReDialog", () => ({ addDialog, closeDialog: vi.fn() }));
vi.mock("@/api/notification", () => ({
  listAnnouncements,
  createAnnouncement,
  publishAnnouncement,
  revokeAnnouncement
}));
vi.mock("@/utils/sso", () => ({}));
vi.mock("@/router", () => ({}));

import { useAdmin } from "./hook";

function makeTableRef() {
  return { value: { getTableRef: () => ({ clearSelection: vi.fn() }) } };
}

describe("useAdmin shape", () => {
  it("returns expected API keys", () => {
    const api = useAdmin();
    const expected = [
      "form",
      "loading",
      "columns",
      "dataList",
      "pagination",
      "onSearch",
      "openDialog",
      "handlePublish",
      "handleRevoke"
    ];
    for (const k of expected) {
      expect(api).toHaveProperty(k);
    }
  });

  it("columns includes 4 base columns", () => {
    const api = useAdmin();
    expect(api.columns.length).toBe(4);
    const labels = api.columns.map((c: any) => c.label);
    expect(labels).toEqual(["ID", "标题", "状态", "发布时间"]);
  });
});

describe("useAdmin onSearch", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    ElMessage.error.mockReset();
  });

  it("filters list by keyword and status from form", async () => {
    listAnnouncements.mockResolvedValue([
      { id: 1, title: "公告A", status: "draft", publish_at: "2026-01-01" },
      { id: 2, title: "公告B", status: "published", publish_at: "2026-01-02" },
      { id: 3, title: "其他C", status: "published", publish_at: "2026-01-03" }
    ]);
    const api = useAdmin();
    api.form.keyword = "公告";
    api.form.status = "published";
    await api.onSearch();
    expect(listAnnouncements).toHaveBeenCalled();
    expect(api.dataList.value).toHaveLength(1);
    expect(api.dataList.value.every((a: any) => a.status === "published")).toBe(
      true
    );
    expect(api.pagination.total).toBe(1);
  });

  it("on API error shows ElMessage.error and sets loading false", async () => {
    listAnnouncements.mockRejectedValue(new Error("network"));
    const api = useAdmin();
    await api.onSearch();
    expect(ElMessage.error).toHaveBeenCalledWith(
      expect.stringContaining("加载失败")
    );
    expect(api.loading.value).toBe(false);
  });
});

describe("useAdmin handlePublish", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    publishAnnouncement.mockReset();
  });

  it("calls publishAnnouncement and reloads on success", async () => {
    listAnnouncements.mockResolvedValue([]);
    publishAnnouncement.mockResolvedValue({});
    const api = useAdmin();
    await api.handlePublish({ id: 42, title: "test" });
    expect(publishAnnouncement).toHaveBeenCalledWith(42);
    expect(ElMessage.success).toHaveBeenCalled();
    expect(listAnnouncements).toHaveBeenCalled();
  });

  it("on API error shows ElMessage.error", async () => {
    listAnnouncements.mockResolvedValue([]);
    publishAnnouncement.mockRejectedValue(new Error("oops"));
    const api = useAdmin();
    await api.handlePublish({ id: 1 });
    expect(ElMessage.error).toHaveBeenCalledWith(
      expect.stringContaining("发布失败")
    );
  });
});

describe("useAdmin handleRevoke", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    ElMessageBox.confirm.mockReset();
    ElMessageBox.confirm.mockResolvedValue("confirm");
  });

  it("calls revokeAnnouncement after confirm", async () => {
    listAnnouncements.mockResolvedValue([]);
    revokeAnnouncement.mockResolvedValue({});
    const api = useAdmin();
    await api.handleRevoke({ id: 7, title: "公告X" });
    expect(ElMessageBox.confirm).toHaveBeenCalled();
    expect(revokeAnnouncement).toHaveBeenCalledWith(7);
    expect(ElMessage.success).toHaveBeenCalled();
  });

  it("on confirm cancel does not call revoke", async () => {
    listAnnouncements.mockResolvedValue([]);
    ElMessageBox.confirm.mockRejectedValueOnce("cancel" as any);
    revokeAnnouncement.mockResolvedValue({});
    const api = useAdmin();
    await api.handleRevoke({ id: 7, title: "公告X" });
    expect(revokeAnnouncement).not.toHaveBeenCalled();
  });
});
