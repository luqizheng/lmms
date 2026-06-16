import dayjs from "dayjs";
import editForm from "../form.vue";
import { message } from "@/utils/message";
import { ElMessageBox } from "element-plus";
import { addDialog } from "@/components/ReDialog";
import type { PermissionFormItem } from "../utils/types";
import type { PaginationProps } from "@pureadmin/table";
import { reactive, ref, onMounted, h } from "vue";
import {
  listIamPermissions,
  createIamPermission,
  updateIamPermission,
  deleteIamPermission,
  type IamPermission
} from "@/api/iam";

export function usePermission() {
  const form = reactive({
    keyword: "",
    category: undefined as number | undefined
  });
  const formRef = ref();
  const dataList = ref<IamPermission[]>([]);
  const loading = ref(true);
  const pagination = reactive<PaginationProps>({
    total: 0,
    pageSize: 10,
    currentPage: 1,
    background: true
  });

  const columns: TableColumnList = [
    { label: "权限码", prop: "code", minWidth: 180 },
    { label: "名称", prop: "name", minWidth: 140 },
    { label: "类别", prop: "category", minWidth: 90 },
    { label: "资源", prop: "resource", minWidth: 120 },
    { label: "动作", prop: "action", minWidth: 100 },
    { label: "排序", prop: "sort_order", width: 80 },
    {
      label: "状态",
      prop: "status",
      width: 90,
      cellRenderer: ({ row }) =>
        h(
          "el-tag",
          { type: row.status === "active" ? "success" : "info" },
          { default: () => (row.status === "active" ? "启用" : "禁用") }
        )
    },
    {
      label: "更新时间",
      prop: "updated_at",
      minWidth: 170,
      formatter: ({ updated_at }) =>
        updated_at ? dayjs(updated_at).format("YYYY-MM-DD HH:mm:ss") : "-"
    },
    { label: "操作", fixed: "right", width: 180, slot: "operation" }
  ];

  async function onSearch() {
    loading.value = true;
    try {
      const res = await listIamPermissions({
        keyword: form.keyword || undefined,
        category: form.category,
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

  function categoryToInt(category: string | number): number {
    if (typeof category === "number") return category;
    if (category === "menu") return 0;
    if (category === "button") return 1;
    if (category === "api") return 2;
    return 1;
  }

  function openDialog(title = "新增", row?: IamPermission) {
    const isEdit = title === "修改";
    const formInline: PermissionFormItem = {
      id: row?.id,
      code: row?.code ?? "",
      name: row?.name ?? "",
      description: row?.description ?? "",
      category: row ? categoryToInt(row.category) : 1,
      resource: row?.resource ?? "",
      action: row?.action ?? "",
      sort_order: row?.sort_order ?? 0,
      status: row?.status ?? "active"
    };
    addDialog({
      title: `${title}权限`,
      props: { formInline, isEdit },
      width: "560px",
      draggable: true,
      closeOnClickModal: false,
      contentRenderer: () =>
        h(editForm, { ref: formRef, formInline: null, isEdit }),
      beforeSure: (done, { options }) => {
        const FormRef = formRef.value.getRef();
        const cur = options.props.formInline as PermissionFormItem;
        FormRef.validate(async valid => {
          if (!valid) return;
          try {
            if (!isEdit) {
              await createIamPermission({
                code: cur.code,
                name: cur.name,
                description: cur.description,
                category: cur.category,
                resource: cur.resource,
                action: cur.action,
                sort_order: cur.sort_order
              });
              message("新增成功", { type: "success" });
            } else {
              await updateIamPermission(cur.id!, {
                name: cur.name,
                description: cur.description,
                sort_order: cur.sort_order,
                status: cur.status
              });
              message("修改成功", { type: "success" });
            }
            done();
            onSearch();
          } catch (e: any) {
            message(`操作失败：${e?.message || e}`, { type: "error" });
          }
        });
      }
    });
  }

  async function handleDelete(row: IamPermission) {
    try {
      await ElMessageBox.confirm(`确认删除权限 ${row.code}？`, "系统提示", {
        type: "warning"
      });
      await deleteIamPermission(row.id);
      message("删除成功", { type: "success" });
      onSearch();
    } catch (e: any) {
      if (e === "cancel") return;
      message(`删除失败：${e?.message || e}`, { type: "error" });
    }
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
    openDialog,
    handleDelete,
    handleSizeChange,
    handleCurrentChange,
    handleSelectionChange
  };
}
