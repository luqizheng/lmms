import dayjs from "dayjs";
import { message } from "@/utils/message";
import type { PaginationProps } from "@pureadmin/table";
import { reactive, ref, onMounted, h } from "vue";
import { listLoginLogs, type LoginLog } from "@/api/audit";

export function useLoginLog() {
  const dataList = ref<LoginLog[]>([]);
  const loading = ref(true);
  const pagination = reactive<PaginationProps>({
    total: 0,
    pageSize: 10,
    currentPage: 1,
    background: true
  });

  const columns: TableColumnList = [
    { label: "ID", prop: "id", width: 80 },
    { label: "用户名", prop: "username", minWidth: 140 },
    { label: "用户ID", prop: "user_id", minWidth: 140 },
    {
      label: "结果",
      prop: "success",
      minWidth: 90,
      cellRenderer: ({ row }) =>
        h(
          "el-tag",
          { type: row.success ? "success" : "danger" },
          { default: () => (row.success ? "成功" : "失败") }
        )
    },
    { label: "失败原因", prop: "fail_reason", minWidth: 180 },
    { label: "IP", prop: "ip_address", minWidth: 130 },
    { label: "位置", prop: "location", minWidth: 140 },
    { label: "User-Agent", prop: "user_agent", minWidth: 220 },
    {
      label: "时间",
      prop: "created_at",
      minWidth: 170,
      formatter: ({ created_at }) =>
        created_at ? dayjs(created_at).format("YYYY-MM-DD HH:mm:ss") : "-"
    }
  ];

  async function onSearch() {
    loading.value = true;
    try {
      const res = await listLoginLogs({
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

  onMounted(onSearch);

  return {
    loading,
    columns,
    dataList,
    pagination,
    onSearch,
    handleSizeChange,
    handleCurrentChange,
    handleSelectionChange
  };
}
