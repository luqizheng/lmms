import dayjs from "dayjs";
import { reactive, ref, onMounted } from "vue";
import { ElMessage } from "element-plus";
import { useRouter } from "vue-router";
import { listAnnouncements, markRead } from "@/api/notification";
import { useNotificationStore } from "@/store/modules/notification";
import type { InboxRow } from "./types";

export function useInbox() {
  const router = useRouter();
  const store = useNotificationStore();
  const loading = ref(false);
  const dataList = ref<InboxRow[]>([]);
  const form = reactive({ keyword: "" });

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

  async function onSearch() {
    loading.value = true;
    try {
      const list = await listAnnouncements();
      dataList.value = (list || [])
        .filter((a: any) => !form.keyword || a.title.includes(form.keyword))
        .map((a: any) => ({
          id: a.id,
          title: a.title,
          publish_at: a.publish_at,
          status: a.status,
          unread: true
        }));
    } catch (e: any) {
      ElMessage.error(`加载失败：${e?.message ?? e}`);
    } finally {
      loading.value = false;
    }
  }

  async function openDetail(row: InboxRow) {
    try {
      await markRead(row.id);
      store.readOne(row.id);
    } catch {
      // ignore
    }
    router.push(`/notification/detail/${row.id}`);
  }

  onMounted(onSearch);
  return { form, loading, columns, dataList, onSearch, openDetail };
}
