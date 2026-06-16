import dayjs from "dayjs";
import editForm from "../form.vue";
import { message } from "@/utils/message";
import { addDialog } from "@/components/ReDialog";
import { reactive, ref, onMounted, h } from "vue";
import type { FormItemProps } from "../utils/types";
import { cloneDeep, isAllEmpty, deviceDetection } from "@pureadmin/utils";
import {
  listIamOrgUnits,
  createIamOrgUnit,
  updateIamOrgUnit,
  deleteIamOrgUnit,
  type IamOrgUnit,
  type CreateIamOrgUnitRequest,
  type UpdateIamOrgUnitRequest
} from "@/api/iam";

export function useDept() {
  const form = reactive({
    name: "",
    status: "" as string
  });

  const formRef = ref();
  const dataList = ref<IamOrgUnit[]>([]);
  const loading = ref(true);

  const columns: TableColumnList = [
    {
      label: "部门名称",
      prop: "name",
      width: 200,
      align: "left"
    },
    {
      label: "编码",
      prop: "code",
      minWidth: 140
    },
    {
      label: "层级",
      prop: "level",
      width: 80
    },
    {
      label: "排序",
      prop: "sort_order",
      width: 80
    },
    {
      label: "状态",
      prop: "status",
      width: 100
    },
    {
      label: "创建时间",
      minWidth: 180,
      prop: "created_at",
      formatter: ({ created_at }) =>
        created_at ? dayjs(created_at).format("YYYY-MM-DD HH:mm:ss") : ""
    },
    {
      label: "更新时间",
      minWidth: 180,
      prop: "updated_at",
      formatter: ({ updated_at }) =>
        updated_at ? dayjs(updated_at).format("YYYY-MM-DD HH:mm:ss") : ""
    },
    {
      label: "操作",
      fixed: "right",
      width: 210,
      slot: "operation"
    }
  ];

  function handleSelectionChange(val) {
    console.log("handleSelectionChange", val);
  }

  function resetForm(formEl) {
    if (!formEl) return;
    formEl.resetFields();
    onSearch();
  }

  /** 在树上按 name / status 过滤，保留命中节点的祖先链 */
  function filterTree(
    list: IamOrgUnit[],
    keyword: string,
    status: string
  ): IamOrgUnit[] {
    const result: IamOrgUnit[] = [];
    for (const node of list) {
      const children = node.children
        ? filterTree(node.children, keyword, status)
        : [];
      const matchName = !keyword || node.name.includes(keyword);
      const matchStatus = !status || node.status === status;
      if ((matchName && matchStatus) || children.length > 0) {
        result.push({ ...node, children });
      }
    }
    return result;
  }

  async function onSearch() {
    loading.value = true;
    try {
      const { items } = await listIamOrgUnits();
      const tree = items ?? [];
      const hasFilter = !isAllEmpty(form.name) || !isAllEmpty(form.status);
      dataList.value = hasFilter
        ? filterTree(tree, form.name, form.status)
        : tree;
    } finally {
      setTimeout(() => {
        loading.value = false;
      }, 300);
    }
  }

  /** 将树数据转为级联选择器选项，停用节点禁选 */
  function formatHigherDeptOptions(treeList: IamOrgUnit[]) {
    if (!treeList || !treeList.length) return [];
    const newTreeList = [];
    for (let i = 0; i < treeList.length; i++) {
      newTreeList.push({
        ...treeList[i],
        disabled: treeList[i].status !== "active",
        children: formatHigherDeptOptions(treeList[i].children || [])
      });
    }
    return newTreeList;
  }

  function openDialog(
    title = "新增",
    row?: Partial<FormItemProps> & IamOrgUnit
  ) {
    addDialog({
      title: `${title}部门`,
      props: {
        formInline: {
          higherDeptOptions: formatHigherDeptOptions(cloneDeep(dataList.value)),
          id: row?.id,
          parentId: (row as any)?.parent_id ?? row?.parentId ?? "",
          name: row?.name ?? "",
          code: (row as any)?.code ?? "",
          sortOrder: (row as any)?.sort_order ?? row?.sortOrder ?? 0,
          status: (row as any)?.status ?? "active"
        } as FormItemProps
      },
      width: "40%",
      draggable: true,
      fullscreen: deviceDetection(),
      fullscreenIcon: true,
      closeOnClickModal: false,
      contentRenderer: () => h(editForm, { ref: formRef, formInline: null }),
      beforeSure: (done, { options }) => {
        const FormRef = formRef.value.getRef();
        const curData = options.props.formInline as FormItemProps;
        FormRef.validate(async valid => {
          if (!valid) return;
          try {
            if (title === "新增") {
              const payload: CreateIamOrgUnitRequest = {
                parent_id: curData.parentId || undefined,
                name: curData.name,
                code: curData.code,
                sort_order: curData.sortOrder
              };
              await createIamOrgUnit(payload);
            } else {
              const payload: UpdateIamOrgUnitRequest = {
                parent_id: curData.parentId || undefined,
                name: curData.name,
                code: curData.code,
                sort_order: curData.sortOrder,
                status: curData.status
              };
              await updateIamOrgUnit(curData.id!, payload);
            }
            message(`您${title}了部门名称为${curData.name}的这条数据`, {
              type: "success"
            });
            done();
            onSearch();
          } catch (e: any) {
            message(e?.message ?? `${title}失败`, { type: "error" });
          }
        });
      }
    });
  }

  async function handleDelete(row: IamOrgUnit) {
    try {
      await deleteIamOrgUnit(row.id);
      message(`您删除了部门名称为${row.name}的这条数据`, { type: "success" });
      onSearch();
    } catch (e: any) {
      message(e?.message ?? "删除失败", { type: "error" });
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
    /** 搜索 */
    onSearch,
    /** 重置 */
    resetForm,
    /** 新增、修改部门 */
    openDialog,
    /** 删除部门 */
    handleDelete,
    handleSelectionChange
  };
}
