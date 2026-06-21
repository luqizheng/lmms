import dayjs from "dayjs";
import { reactive, ref, onMounted, h } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { addDialog } from "@/components/ReDialog";
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

  const columns: TableColumnList = [
    { label: "ID", prop: "id", width: 60 },
    { label: "标题", prop: "title", minWidth: 200 },
    {
      label: "状态",
      prop: "status",
      width: 100,
      cellRenderer: ({ row }: any) => (
        <el-tag type={row.status === "published" ? "success" : "info"}>
          {row.status}
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
      publish_at:
        row?.publish_at ?? dayjs().format("YYYY-MM-DDTHH:mm:ss[Z]"),
      expire_at: row?.expire_at ?? null,
      pinned: row?.pinned ?? false,
      popover: row?.popover ?? false,
      recipients: []
    };
    let formRef: any = null;
    addDialog({
      title: isEdit ? "修改公告" : "新建公告",
      width: "640px",
      draggable: true,
      closeOnClickModal: false,
      props: { formInline, isEdit },
      contentRenderer: ({ options }) =>
        h(editForm, {
          ref: (el: any) => { formRef = el; },
          formInline: options.props.formInline,
          isEdit
        }),
      beforeSure: async done => {
        if (!formRef) return;
        const cur = formRef.getRef()?.newFormInline;
        if (!cur) return;
        try {
          if (!isEdit) {
            await createAnnouncement({
              title: cur.title,
              body: cur.body,
              publish_at: cur.publish_at,
              expire_at: cur.expire_at,
              pinned: cur.pinned,
              popover: cur.popover,
              recipients: cur.recipients.map((r: any) => ({
                scope_type: r.scope_type,
                scope_id: r.scope_id
              }))
            });
            ElMessage.success("创建成功");
          } else {
            ElMessage.warning("修改功能未实现，请用发布/撤销控制状态");
          }
          done();
          onSearch();
        } catch (e: any) {
          ElMessage.error(`操作失败：${e?.message ?? e}`);
        }
      }
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
