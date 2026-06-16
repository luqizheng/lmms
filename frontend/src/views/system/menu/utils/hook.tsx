import dayjs from "dayjs";
import editForm from "../form.vue";
import { message } from "@/utils/message";
import { addDialog } from "@/components/ReDialog";
import { reactive, ref, onMounted, h } from "vue";
import type { FormItemProps } from "../utils/types";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { cloneDeep, isAllEmpty, deviceDetection } from "@pureadmin/utils";
import {
  listIamMenus,
  createIamMenu,
  updateIamMenu,
  deleteIamMenu,
  type IamMenu,
  type CreateIamMenuRequest,
  type UpdateIamMenuRequest
} from "@/api/iam";

export function useMenu() {
  const form = reactive({
    name: ""
  });

  const formRef = ref();
  const dataList = ref<IamMenu[]>([]);
  const loading = ref(true);

  const columns: TableColumnList = [
    {
      label: "菜单名称",
      prop: "name",
      align: "left",
      cellRenderer: ({ row }) => (
        <>
          {row.icon ? (
            <span class="inline-block mr-1">
              {h(useRenderIcon(row.icon), {
                style: { paddingTop: "1px" }
              })}
            </span>
          ) : null}
          <span>{row.name}</span>
        </>
      )
    },
    {
      label: "菜单编码",
      prop: "code"
    },
    {
      label: "路由路径",
      prop: "path"
    },
    {
      label: "组件路径",
      prop: "component",
      formatter: ({ path, component }) =>
        isAllEmpty(component) ? path : component
    },
    {
      label: "排序",
      prop: "sort_order",
      width: 100
    },
    {
      label: "可见",
      prop: "visible",
      width: 90,
      formatter: ({ visible }) => (visible ? "是" : "否")
    },
    {
      label: "状态",
      prop: "status",
      width: 100
    },
    {
      label: "更新时间",
      prop: "updated_at",
      minWidth: 180,
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

  /** 在树上按 name 模糊过滤，保留命中节点的祖先链 */
  function filterTree(list: IamMenu[], keyword: string): IamMenu[] {
    const result: IamMenu[] = [];
    for (const node of list) {
      const children = node.children ? filterTree(node.children, keyword) : [];
      if (node.name.includes(keyword) || children.length > 0) {
        result.push({ ...node, children });
      }
    }
    return result;
  }

  async function onSearch() {
    loading.value = true;
    try {
      const { items } = await listIamMenus();
      const tree = items ?? [];
      dataList.value = isAllEmpty(form.name)
        ? tree
        : filterTree(tree, form.name);
    } finally {
      setTimeout(() => {
        loading.value = false;
      }, 300);
    }
  }

  /** 将树数据转为级联选择器选项（仅保留 id/name/children 等字段） */
  function formatHigherMenuOptions(treeList: IamMenu[]) {
    if (!treeList || !treeList.length) return [];
    const newTreeList = [];
    for (let i = 0; i < treeList.length; i++) {
      newTreeList.push({
        ...treeList[i],
        children: formatHigherMenuOptions(treeList[i].children || [])
      });
    }
    return newTreeList;
  }

  function openDialog(title = "新增", row?: Partial<FormItemProps> & IamMenu) {
    addDialog({
      title: `${title}菜单`,
      props: {
        formInline: {
          // menuType 仅用于 UI 控制显示，不发送后端
          menuType: 0,
          higherMenuOptions: formatHigherMenuOptions(cloneDeep(dataList.value)),
          id: row?.id,
          parentId: (row as any)?.parent_id ?? row?.parentId ?? "",
          name: row?.name ?? "",
          code: (row as any)?.code ?? "",
          path: row?.path ?? "",
          component: row?.component ?? "",
          sortOrder: (row as any)?.sort_order ?? row?.sortOrder ?? 0,
          icon: row?.icon ?? "",
          visible: (row as any)?.visible ?? true,
          permissionId: (row as any)?.permission_id ?? row?.permissionId ?? "",
          status: (row as any)?.status ?? "active"
        } as FormItemProps
      },
      width: "45%",
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
              const payload: CreateIamMenuRequest = {
                parent_id: curData.parentId || undefined,
                name: curData.name,
                code: curData.code,
                path: curData.path,
                component: curData.component || undefined,
                icon: curData.icon || undefined,
                sort_order: curData.sortOrder,
                visible: curData.visible,
                permission_id: curData.permissionId || undefined
              };
              await createIamMenu(payload);
            } else {
              const payload: UpdateIamMenuRequest = {
                parent_id: curData.parentId || undefined,
                name: curData.name,
                code: curData.code,
                path: curData.path,
                component: curData.component || undefined,
                icon: curData.icon || undefined,
                sort_order: curData.sortOrder,
                visible: curData.visible,
                permission_id: curData.permissionId || undefined,
                status: curData.status
              };
              await updateIamMenu(curData.id!, payload);
            }
            message(`您${title}了菜单名称为${curData.name}的这条数据`, {
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

  async function handleDelete(row: IamMenu) {
    try {
      await deleteIamMenu(row.id);
      message(`您删除了菜单名称为${row.name}的这条数据`, { type: "success" });
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
    /** 新增、修改菜单 */
    openDialog,
    /** 删除菜单 */
    handleDelete,
    handleSelectionChange
  };
}
