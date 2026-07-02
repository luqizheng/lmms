import dayjs from "dayjs";
import { reactive, ref, computed, onMounted } from "vue";
import { ElMessage } from "element-plus";
import { useRouter } from "vue-router";
import { listAnnouncements, markRead } from "@/api/notification";
import { useNotificationStore } from "@/store/modules/notification";
import type { InboxRow } from "./types";
import { useInboxReadState } from "./read-state";

export type InboxTab = "all" | "unread" | "read";

export function useInbox() {
  const router = useRouter();
  const store = useNotificationStore();
  const loading = ref(false);
  const dataList = ref<InboxRow[]>([]);
  const form = reactive({ keyword: "" });
  const activeTab = ref<InboxTab>("all");
  const readState = useInboxReadState();

  const columns: TableColumnList = [
    { label: "标题", prop: "title", minWidth: 220 },
    {
      label: "状态",
      prop: "status",
      width: 90,
      cellRenderer: ({ row }: any) => (
        <el-tag type={row.unread ? "danger" : "info"}>
          {row.unread ? "未读" : "已读"}
        </el-tag>
      )
    },
    {
      label: "发布时间",
      prop: "publish_at",
      width: 170,
      formatter: ({ publish_at }: any) =>
        publish_at ? dayjs(publish_at).format("YYYY-MM-DD HH:mm") : "-"
    }
  ];

  function applyTabFilter(rows: InboxRow[]): InboxRow[] {
    if (activeTab.value === "all") return rows;
    const wantUnread = activeTab.value === "unread";
    return rows.filter(r => r.unread === wantUnread);
  }

  async function onSearch() {
    loading.value = true;
    try {
      const list = await listAnnouncements();
      const rows: InboxRow[] = (list || [])
        .filter((a: any) => !form.keyword || a.title.includes(form.keyword))
        .map((a: any) => ({
          id: a.id,
          title: a.title,
          publish_at: a.publish_at,
          status: a.status,
          unread: !readState.isRead(a.id)
        }));
      dataList.value = applyTabFilter(rows);
    } catch (e: any) {
      ElMessage.error(`加载失败：${e?.message ?? e}`);
    } finally {
      loading.value = false;
    }
  }

  async function openDetail(row: InboxRow) {
    try {
      await markRead(row.id);
      readState.markRead(row.id);
      store.readOne(row.id);
    } catch {
      // ignore
    }
    router.push(`/notification/detail/${row.id}`);
  }

  function onTabChange(tab: InboxTab | string) {
    activeTab.value = tab as InboxTab;
    onSearch();
  }

  const unreadCount = computed(
    () => dataList.value.filter(r => r.unread).length
  );

  onMounted(onSearch);
  return {
    form,
    loading,
    columns,
    dataList,
    activeTab,
    unreadCount,
    onSearch,
    onTabChange,
    openDetail
  };
}
