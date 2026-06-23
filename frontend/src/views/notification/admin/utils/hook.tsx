import dayjs from "dayjs";
import { reactive, ref, onMounted, h } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useDebounceFn } from "@vueuse/core";
import { addDialog, closeDialog } from "@/components/ReDialog";
import {
  listAnnouncements,
  createAnnouncement,
  publishAnnouncement,
  revokeAnnouncement
} from "@/api/notification";
import editForm from "../form.vue";
import type { FormItemProps } from "../utils/types";

export function useAdmin() {
  const loading = ref(true);
  const dataList = ref<any[]>([]);
  const pagination = reactive({
    total: 0,
    pageSize: 10,
    currentPage: 1,
    background: true
  });
  const form = reactive({ keyword: "", status: "" });

  const statusMap: Record<
    string,
    {
      label: string;
      type: "primary" | "success" | "warning" | "info" | "danger";
    }
  > = {
    draft: { label: "草稿", type: "info" },
    published: { label: "已发布", type: "success" },
    revoked: { label: "已撤销", type: "danger" }
  };

  const columns: TableColumnList = [
    { label: "ID", prop: "id", width: 60 },
    { label: "标题", prop: "title", minWidth: 200 },
    {
      label: "状态",
      prop: "status",
      width: 100,
      cellRenderer: ({ row }: any) => {
        const status = statusMap[row.status] || {
          label: row.status,
          type: "info"
        };
        return <el-tag type={status.type}>{status.label}</el-tag>;
      }
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
      dataList.value = (list || []).filter(
        (a: any) =>
          (!form.keyword || a.title.includes(form.keyword)) &&
          (!form.status || a.status === form.status)
      );
      pagination.total = dataList.value.length;
    } catch (e: any) {
      ElMessage.error(`加载失败：${e?.message ?? e}`);
    } finally {
      loading.value = false;
    }
  }

  async function openDialog(row?: any) {
    const isEdit = !!row;
    const formInline: FormItemProps = {
      id: row?.id,
      title: row?.title ?? "",
      body: row?.body ?? "",
      publish_at: row?.publish_at ?? dayjs().format("YYYY-MM-DDTHH:mm:ss[Z]"),
      expire_at: row?.expire_at ?? null,
      pinned: row?.pinned ?? false,
      popover: row?.popover ?? false,
      recipients: []
    };
    let formRef: any = null;
    const submitPublish = useDebounceFn(async (options: any, index: number) => {
      if (!formRef) return;
      const cur = formRef.newFormInline;
      if (!cur) return;
      try {
        let id = cur.id;
          if (!isEdit) {
            const recipients = (cur.recipients || [])
              .filter((r: any) => r?.scope_id != null && r.scope_id !== "")
              .map((r: any) => ({
                scope_type: r.scope_type,
                scope_id: r.scope_id
              }));
            if (recipients.length === 0) {
              ElMessage.error("请至少选择一个有效的接收人");
              return;
            }
            const created = await createAnnouncement({
              title: cur.title,
              body: cur.body,
              publish_at: cur.publish_at,
              expire_at: cur.expire_at,
              pinned: cur.pinned,
              popover: cur.popover,
              recipients
            });
            id = (created as any)?.id ?? id;
          }
        if (!id) {
          ElMessage.error("缺少公告 ID，无法发布");
          return;
        }
        await publishAnnouncement(id);
        ElMessage.success("已发布");
        closeDialog(options, index);
        onSearch();
      } catch (e: any) {
        ElMessage.error(`发布失败：${e?.message ?? e}`);
      }
    }, 300);
    addDialog({
      title: isEdit ? "修改公告" : "新建公告",
      width: "640px",
      draggable: true,
      closeOnClickModal: false,
      props: { formInline, isEdit },
      contentRenderer: ({ options }) =>
        h(editForm, {
          ref: (el: any) => {
            formRef = el;
          },
          formInline: options.props.formInline,
          isEdit
        }),
      footerButtons: [
        {
          label: "取消",
          btnClick: ({ dialog: { options, index } }) => {
            closeDialog(options, index);
          }
        },
        {
          label: "发布",
          type: "primary",
          btnClick: ({ dialog: { options, index } }) => {
            submitPublish(options, index);
          }
        }
      ]
    });
  }

  async function handlePublish(row: any) {
    try {
      await publishAnnouncement(row.id);
      ElMessage.success("已发布");
      onSearch();
    } catch (e: any) {
      ElMessage.error(`发布失败：${e?.message ?? e}`);
    }
  }

  async function handleRevoke(row: any) {
    try {
      await ElMessageBox.confirm(`确认撤销 ${row.title} ?`, "系统提示", {
        type: "warning"
      });
      await revokeAnnouncement(row.id);
      ElMessage.success("已撤销");
      onSearch();
    } catch (e: any) {
      if (e === "cancel") return;
      ElMessage.error(`撤销失败：${e?.message ?? e}`);
    }
  }

  onMounted(() => {
    onSearch();
  });

  return {
    form,
    loading,
    columns,
    dataList,
    pagination,
    onSearch,
    openDialog,
    handlePublish,
    handleRevoke
  };
}
