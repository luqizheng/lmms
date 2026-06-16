import "./reset.css";
import dayjs from "dayjs";
import roleForm from "../form/role.vue";
import editForm from "../form/index.vue";
import { message } from "@/utils/message";
import userAvatar from "@/assets/user.jpg";
import { usePublicHooks } from "../../hooks";
import { addDialog } from "@/components/ReDialog";
import type { PaginationProps } from "@pureadmin/table";
import ReCropperPreview from "@/components/ReCropperPreview";
import type { FormItemProps, RoleFormItemProps } from "../utils/types";
import { getKeyList, hideTextAtIndex, deviceDetection } from "@pureadmin/utils";
import {
  listIamUsers,
  createIamUser,
  updateIamUser,
  deleteIamUser,
  listIamRoles,
  type IamUser
} from "@/api/iam";
import { type Ref, h, ref, computed, reactive, onMounted } from "vue";

export function useUser(tableRef: Ref, _treeRef: Ref) {
  const form = reactive({
    // 左侧部门树的 id（保留字段以兼容 index.vue，IAM 暂不使用）
    deptId: "",
    username: "",
    phone: "",
    status: ""
  });
  const formRef = ref();
  const dataList = ref<IamUser[]>([]);
  const loading = ref(true);
  // 上传头像信息
  const avatarInfo = ref();
  const switchLoadMap = ref({});
  const { switchStyle } = usePublicHooks();
  const higherDeptOptions = ref<any[]>([]);
  const treeData = ref<any[]>([]);
  const treeLoading = ref(false);
  const selectedNum = ref(0);
  const pagination = reactive<PaginationProps>({
    total: 0,
    pageSize: 10,
    currentPage: 1,
    background: true
  });
  const columns: TableColumnList = [
    {
      label: "勾选列",
      type: "selection",
      fixed: "left",
      reserveSelection: true
    },
    {
      label: "用户ID",
      prop: "id",
      width: 90
    },
    {
      label: "头像",
      prop: "avatar",
      cellRenderer: ({ row }) => (
        <el-image
          fit="cover"
          preview-teleported={true}
          src={row.avatar || userAvatar}
          preview-src-list={Array.of(row.avatar || userAvatar)}
          class="size-6 rounded-full align-middle"
        />
      ),
      width: 90
    },
    {
      label: "用户名",
      prop: "username",
      minWidth: 130
    },
    {
      label: "昵称",
      prop: "nickname",
      minWidth: 130
    },
    {
      label: "邮箱",
      prop: "email",
      minWidth: 160
    },
    {
      label: "手机号",
      prop: "phone",
      minWidth: 110,
      formatter: ({ phone }) =>
        phone ? hideTextAtIndex(phone, { start: 3, end: 6 }) : ""
    },
    {
      label: "状态",
      prop: "status",
      minWidth: 100,
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
      )
    },
    {
      label: "最后登录",
      prop: "last_login_at",
      minWidth: 160,
      formatter: ({ last_login_at }) =>
        last_login_at ? dayjs(last_login_at).format("YYYY-MM-DD HH:mm:ss") : "-"
    },
    {
      label: "创建时间",
      minWidth: 160,
      prop: "created_at",
      formatter: ({ created_at }) =>
        created_at ? dayjs(created_at).format("YYYY-MM-DD HH:mm:ss") : "-"
    },
    {
      label: "操作",
      fixed: "right",
      width: 180,
      slot: "operation"
    }
  ];
  const buttonClass = computed(() => {
    return [
      "h-5!",
      "reset-margin",
      "text-gray-500!",
      "dark:text-white!",
      "dark:hover:text-primary!"
    ];
  });
  const roleOptions = ref<{ id: string; name: string }[]>([]);

  async function onStatusChange(
    scope: { row: IamUser; index: number },
    val: boolean
  ) {
    const { row, index } = scope;
    const original = row.status;
    const next = val ? "active" : "disabled";
    switchLoadMap.value[index] = { loading: true };
    try {
      await updateIamUser(row.id, { status: next });
      row.status = next;
      message("已修改用户状态", { type: "success" });
    } catch (e: any) {
      row.status = original;
      message(`修改用户状态失败：${e?.message || e}`, { type: "error" });
    } finally {
      switchLoadMap.value[index] = { loading: false };
    }
  }

  function handleUpdate(row) {
    console.log(row);
  }

  async function handleDelete(row: IamUser) {
    try {
      await deleteIamUser(row.id);
      message(`已删除用户 ${row.username}`, { type: "success" });
      onSearch();
    } catch (e: any) {
      message(`删除用户失败：${e?.message || e}`, { type: "error" });
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

  /** 当 CheckBox 选择项发生变化时会触发该事件 */
  function handleSelectionChange(val) {
    selectedNum.value = val.length;
    tableRef.value?.setAdaptive?.();
  }

  /** 取消选择 */
  function onSelectionCancel() {
    selectedNum.value = 0;
    tableRef.value?.getTableRef()?.clearSelection();
  }

  /** 批量删除 */
  async function onbatchDel() {
    const curSelected = tableRef.value.getTableRef().getSelectionRows();
    const ids = getKeyList(curSelected, "id");
    try {
      await Promise.all(ids.map((id: string) => deleteIamUser(id)));
      message(`已批量删除 ${ids.length} 位用户`, { type: "success" });
      tableRef.value.getTableRef().clearSelection();
      onSearch();
    } catch (e: any) {
      message(`批量删除失败：${e?.message || e}`, { type: "error" });
    }
  }

  async function onSearch() {
    loading.value = true;
    try {
      const data = await listIamUsers({
        keyword: form.username || undefined,
        status: form.status || undefined,
        page: pagination.currentPage,
        size: pagination.pageSize
      });
      dataList.value = data?.items ?? [];
      pagination.total = data?.total ?? 0;
      pagination.pageSize = data?.size ?? pagination.pageSize;
      pagination.currentPage = data?.page ?? pagination.currentPage;
    } catch (e: any) {
      message(`加载用户列表失败：${e?.message || e}`, { type: "error" });
    } finally {
      loading.value = false;
    }
  }

  const resetForm = formEl => {
    if (!formEl) return;
    formEl.resetFields();
    form.deptId = "";
    onSearch();
  };

  function onTreeSelect(_payload: { id: string; selected: boolean }) {
    // IAM 暂未提供组织树过滤，保留接口签名
  }

  function openDialog(title = "新增", row?: FormItemProps) {
    addDialog({
      title: `${title}用户`,
      props: {
        formInline: {
          title,
          higherDeptOptions: higherDeptOptions.value,
          parentId: 0,
          nickname: row?.nickname ?? "",
          username: row?.username ?? "",
          password: "",
          phone: (row?.phone as string) ?? "",
          email: row?.email ?? "",
          sex: "",
          status: row?.status ?? "active",
          remark: "",
          avatar: row?.avatar ?? ""
        }
      },
      width: "46%",
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
              await createIamUser({
                username: curData.username,
                password: curData.password,
                nickname: curData.nickname,
                email: curData.email || undefined,
                phone: (curData.phone as string) || undefined,
                avatar: curData.avatar || undefined
              });
              message(`已新增用户 ${curData.username}`, { type: "success" });
            } else if (row?.id) {
              await updateIamUser(row.id, {
                nickname: curData.nickname,
                email: curData.email || undefined,
                phone: (curData.phone as string) || undefined,
                avatar: curData.avatar || undefined,
                status: curData.status
              });
              message(`已修改用户 ${curData.username}`, { type: "success" });
            }
            done();
            onSearch();
          } catch (e: any) {
            message(`${title}用户失败：${e?.message || e}`, { type: "error" });
          }
        });
      }
    });
  }

  const cropRef = ref();
  /** 上传头像 */
  function handleUpload(row) {
    addDialog({
      title: "裁剪、上传头像",
      width: "40%",
      closeOnClickModal: false,
      fullscreen: deviceDetection(),
      contentRenderer: () =>
        h(ReCropperPreview, {
          ref: cropRef,
          imgSrc: row.avatar || userAvatar,
          onCropper: info => (avatarInfo.value = info)
        }),
      beforeSure: done => {
        message("头像上传接口暂未在管理端开放", { type: "warning" });
        done();
      },
      closeCallBack: () => cropRef.value?.hidePopover()
    });
  }

  /** 重置密码 */
  function handleReset(row) {
    message(
      `重置 ${row.username} 用户的密码：该接口由 IAM 提供，暂未在管理端开放`,
      { type: "warning" }
    );
  }

  /** 分配角色 */
  async function handleRole(row: IamUser) {
    addDialog({
      title: `分配 ${row.username} 用户的角色`,
      props: {
        formInline: {
          username: row?.username ?? "",
          nickname: row?.nickname ?? "",
          roleOptions: roleOptions.value ?? [],
          ids: []
        }
      },
      width: "400px",
      draggable: true,
      fullscreen: deviceDetection(),
      fullscreenIcon: true,
      closeOnClickModal: false,
      contentRenderer: () => h(roleForm),
      beforeSure: (done, { options }) => {
        const curData = options.props.formInline as RoleFormItemProps;
        console.log("selected role ids", curData.ids);
        message("已记录所选角色 id（接口暂未提供）", { type: "warning" });
        done();
      }
    });
  }

  onMounted(async () => {
    onSearch();
    try {
      const res = await listIamRoles({ page: 1, size: 1000 });
      roleOptions.value = (res?.items ?? []).map(r => ({
        id: r.id,
        name: r.name
      }));
    } catch (e: any) {
      message(`加载角色列表失败：${e?.message || e}`, { type: "error" });
    }
  });

  return {
    form,
    loading,
    columns,
    dataList,
    treeData,
    treeLoading,
    selectedNum,
    pagination,
    buttonClass,
    deviceDetection,
    onSearch,
    resetForm,
    onbatchDel,
    openDialog,
    onTreeSelect,
    handleUpdate,
    handleDelete,
    handleUpload,
    handleReset,
    handleRole,
    handleSizeChange,
    onSelectionCancel,
    handleCurrentChange,
    handleSelectionChange
  };
}
