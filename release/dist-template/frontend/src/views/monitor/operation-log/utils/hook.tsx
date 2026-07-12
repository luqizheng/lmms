import dayjs from "dayjs";
import { message } from "@/utils/message";
import type { PaginationProps } from "@pureadmin/table";
import { reactive, ref, onMounted, h } from "vue";
import { listOperationLogs, type OperationLog } from "@/api/audit";
import { addDrawer } from "@/components/ReDrawer";

export function useOperationLog() {
  const form = reactive({
    user_id: "",
    action: "",
    resource: ""
  });
  const dataList = ref<OperationLog[]>([]);
  const loading = ref(true);
  const pagination = reactive<PaginationProps>({
    total: 0,
    pageSize: 10,
    currentPage: 1,
    background: true
  });

  const columns: TableColumnList = [
    { label: "ID", prop: "id", width: 80 },
    { label: "用户ID", prop: "user_id", minWidth: 140 },
    { label: "操作", prop: "action", minWidth: 120 },
    { label: "资源", prop: "resource", minWidth: 140 },
    { label: "资源ID", prop: "resource_id", minWidth: 140 },
    { label: "IP", prop: "ip_address", minWidth: 130 },
    { label: "Request ID", prop: "request_id", minWidth: 180 },
    {
      label: "时间",
      prop: "created_at",
      minWidth: 170,
      formatter: ({ created_at }) =>
        created_at ? dayjs(created_at).format("YYYY-MM-DD HH:mm:ss") : "-"
    },
    {
      label: "操作",
      fixed: "right",
      width: 100,
      slot: "operation"
    }
  ];

  async function onSearch() {
    loading.value = true;
    try {
      const res = await listOperationLogs({
        user_id: form.user_id || undefined,
        action: form.action || undefined,
        resource: form.resource || undefined,
        page: pagination.currentPage,
        size: pagination.pageSize
      });
      dataList.value = res.items || [];
      pagination.total = res.total || 0;
    } catch (e: any) {
      message(`加载失败：${e?.message || e}`, { type: "error" });
    } finally {
      loading.value = false;
    }
  }

  function resetForm(formEl) {
    if (!formEl) return;
    formEl.resetFields();
    pagination.currentPage = 1;
    onSearch();
  }

  function handleSizeChange(val: number) {
    pagination.pageSize = val;
    onSearch();
  }
  function handleCurrentChange(val: number) {
    pagination.currentPage = val;
    onSearch();
  }
  function handleSelectionChange(_val) {
    /* noop */
  }

  function showDetail(row: OperationLog) {
    addDrawer({
      title: `操作日志 #${row.id}`,
      size: "640px",
      hideFooter: true,
      contentRenderer: () =>
        h("div", { style: "padding:8px 16px;line-height:1.9" }, [
          h("div", null, [h("b", null, "用户："), row.user_id]),
          h("div", null, [h("b", null, "操作："), row.action]),
          h("div", null, [
            h("b", null, "资源："),
            `${row.resource}${row.resource_id ? " / " + row.resource_id : ""}`
          ]),
          h("div", null, [h("b", null, "Request ID："), row.request_id || "-"]),
          h("div", null, [h("b", null, "IP："), row.ip_address || "-"]),
          h("div", null, [h("b", null, "User-Agent："), row.user_agent || "-"]),
          h("div", null, [
            h("b", null, "时间："),
            row.created_at
              ? dayjs(row.created_at).format("YYYY-MM-DD HH:mm:ss")
              : "-"
          ]),
          h("el-divider", null),
          h("div", { style: "font-weight:bold;margin:6px 0" }, "Before:"),
          h(
            "pre",
            {
              style:
                "background:var(--el-fill-color-light);padding:8px;white-space:pre-wrap;word-break:break-all"
            },
            row.before_value || "-"
          ),
          h("div", { style: "font-weight:bold;margin:6px 0" }, "After:"),
          h(
            "pre",
            {
              style:
                "background:var(--el-fill-color-light);padding:8px;white-space:pre-wrap;word-break:break-all"
            },
            row.after_value || "-"
          )
        ])
    });
  }

  onMounted(onSearch);

  return {
    form,
    loading,
    columns,
    dataList,
    pagination,
    onSearch,
    resetForm,
    showDetail,
    handleSizeChange,
    handleCurrentChange,
    handleSelectionChange
  };
}
