import dayjs from "dayjs";
import editForm from "../form.vue";
import { message } from "@/utils/message";
import { usePublicHooks } from "../../hooks";
import { addDialog } from "@/components/ReDialog";
import type { FormItemProps } from "../utils/types";
import type { PaginationProps } from "@pureadmin/table";
import { deviceDetection } from "@pureadmin/utils";
import {
  listIamRoles,
  createIamRole,
  updateIamRole,
  deleteIamRole,
  listIamPermissions,
  assignRolePermissions,
  type IamRole,
  type IamPermission
} from "@/api/iam";
import { type Ref, reactive, ref, onMounted, h, computed } from "vue";
import { dataScopeText, dataScopeToNumber } from "./data-scope";

export function useRole(_treeRef: Ref) {
  const form = reactive({
    name: "",
    code: "",
    status: ""
  });
  const curRow = ref<IamRole | null>(null);
  const formRef = ref();
  const dataList = ref<IamRole[]>([]);
  const loading = ref(true);
  const switchLoadMap = ref({});
  const { switchStyle } = usePublicHooks();
  const pagination = reactive<PaginationProps>({
    total: 0,
    pageSize: 10,
    currentPage: 1,
    background: true
  });

  // 权限分配弹窗状态
  const permissionDialogVisible = ref(false);
  const allPermissions = ref<IamPermission[]>([]);
  const checkedPermissionIds = ref<string[]>([]);
  const groupedPermissions = computed<Record<string, IamPermission[]>>(() => {
    const groups: Record<string, IamPermission[]> = {};
    for (const p of allPermissions.value) {
      const key = p.category || "其它";
      if (!groups[key]) groups[key] = [];
      groups[key].push(p);
    }
    return groups;
  });

  const columns: TableColumnList = [
    {
      label: "角色编号",
      prop: "id",
      minWidth: 90
    },
    {
      label: "角色名称",
      prop: "name",
      minWidth: 130
    },
    {
      label: "角色标识",
      prop: "code",
      minWidth: 130
    },
    {
      label: "数据范围",
      prop: "data_scope",
      minWidth: 130,
      formatter: ({ data_scope }) => dataScopeText(data_scope)
    },
    {
      label: "状态",
      cellRenderer: scope => (
        <el-switch
          size={scope.props.size === "small" ? "small" : "default"}
          loading={switchLoadMap.value[scope.index]?.loading}
          model-value={scope.row.status === "active"}
          active-text="已启用"
          inactive-text="已停用"
          inline-prompt
          style={switchStyle.value}
          onChange={(val: boolean) => onStatusChange(scope as any, val)}
        />
      ),
      minWidth: 100
    },
    {
      label: "描述",
      prop: "description",
      minWidth: 160
    },
    {
      label: "创建时间",
      prop: "created_at",
      minWidth: 160,
      formatter: ({ created_at }) =>
        created_at ? dayjs(created_at).format("YYYY-MM-DD HH:mm:ss") : "-"
    },
    {
      label: "操作",
      fixed: "right",
      width: 210,
      slot: "operation"
    }
  ];

  async function onStatusChange(
    scope: { row: IamRole; index: number },
    val: boolean
  ) {
    const { row, index } = scope;
    const original = row.status;
    const next = val ? "active" : "disabled";
    switchLoadMap.value[index] = { loading: true };
    try {
      await updateIamRole(row.id, { status: next });
      row.status = next;
      message(`已${next === "active" ? "启用" : "停用"}${row.name}`, {
        type: "success"
      });
    } catch (e: any) {
      row.status = original;
      message(`修改角色状态失败：${e?.message || e}`, { type: "error" });
    } finally {
      switchLoadMap.value[index] = { loading: false };
    }
  }

  async function handleDelete(row: IamRole) {
    try {
      await deleteIamRole(row.id);
      message(`已删除角色 ${row.name}`, { type: "success" });
      onSearch();
    } catch (e: any) {
      message(`删除角色失败：${e?.message || e}`, { type: "error" });
    }
  }

  function handleSizeChange(val: number) {
    pagination.pageSize = val;
    pagination.currentPage = 1;
    onSearch();
  }

  function handleCurrentChange(val: number) {
    pagination.currentPage = val;
    onSearch();
  }

  function handleSelectionChange(_val) {
    // 角色列表暂不需要批量操作
  }

  async function onSearch() {
    loading.value = true;
    try {
      const data = await listIamRoles({
        keyword: form.name || form.code || undefined,
        status: form.status || undefined,
        page: pagination.currentPage,
        size: pagination.pageSize
      });
      dataList.value = data?.items ?? [];
      pagination.total = data?.total ?? 0;
      pagination.pageSize = data?.size ?? pagination.pageSize;
      pagination.currentPage = data?.page ?? pagination.currentPage;
    } catch (e: any) {
      message(`加载角色列表失败：${e?.message || e}`, { type: "error" });
    } finally {
      loading.value = false;
    }
  }

  const resetForm = formEl => {
    if (!formEl) return;
    formEl.resetFields();
    onSearch();
  };

  function openDialog(title = "新增", row?: IamRole) {
    const isCreate = title === "新增";
    addDialog({
      title: `${title}角色`,
      props: {
        formInline: {
          isCreate,
          name: row?.name ?? "",
          code: row?.code ?? "",
          description: row?.description ?? "",
          data_scope: dataScopeToNumber(row?.data_scope),
          status: row?.status ?? "active"
        }
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
            if (isCreate) {
              await createIamRole({
                name: curData.name,
                code: curData.code,
                description: curData.description || undefined,
                data_scope: dataScopeToNumber(curData.data_scope)
              });
              message(`已新增角色 ${curData.name}`, { type: "success" });
            } else if (row?.id) {
              await updateIamRole(row.id, {
                name: curData.name,
                description: curData.description || undefined,
                data_scope: dataScopeToNumber(curData.data_scope),
                status: curData.status
              });
              message(`已修改角色 ${curData.name}`, { type: "success" });
            }
            done();
            onSearch();
          } catch (e: any) {
            message(`${title}角色失败：${e?.message || e}`, { type: "error" });
          }
        });
      }
    });
  }

  /** 权限分配 */
  async function handleMenu(row: IamRole) {
    curRow.value = row;
    try {
      const res = await listIamPermissions({ page: 1, size: 1000 });
      allPermissions.value = res?.items ?? [];
      // 接口暂未提供回显当前角色已有权限，这里默认空
      checkedPermissionIds.value = [];
      permissionDialogVisible.value = true;
    } catch (e: any) {
      message(`加载权限列表失败：${e?.message || e}`, { type: "error" });
    }
  }

  function closePermissionDialog() {
    permissionDialogVisible.value = false;
    curRow.value = null;
    checkedPermissionIds.value = [];
  }

  /** 权限分配-保存 */
  async function handleSave() {
    if (!curRow.value) return;
    try {
      await assignRolePermissions(curRow.value.id, checkedPermissionIds.value);
      message(`角色 ${curRow.value.name} 的权限已更新`, { type: "success" });
      closePermissionDialog();
    } catch (e: any) {
      message(`保存权限失败：${e?.message || e}`, { type: "error" });
    }
  }

  onMounted(() => {
    onSearch();
  });

  return {
    form,
    curRow,
    loading,
    columns,
    dataList,
    pagination,
    permissionDialogVisible,
    groupedPermissions,
    checkedPermissionIds,
    onSearch,
    resetForm,
    openDialog,
    handleMenu,
    handleSave,
    handleDelete,
    closePermissionDialog,
    handleSizeChange,
    handleCurrentChange,
    handleSelectionChange
  };
}
