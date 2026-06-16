import dayjs from "dayjs";
import editForm from "../form.vue";
import { message } from "@/utils/message";
import { ElMessageBox } from "element-plus";
import { addDialog } from "@/components/ReDialog";
import type { FormItemProps } from "../utils/types";
import type { PaginationProps } from "@pureadmin/table";
import { reactive, ref, onMounted, h, toRaw } from "vue";
import {
  listOAuthClients,
  createOAuthClient,
  updateOAuthClient,
  deleteOAuthClient,
  resetOAuthClientSecret,
  type OAuthClient
} from "@/api/token";
import { useClipboard } from "@vueuse/core";

export function useTokenClient() {
  const form = reactive({
    keyword: "",
    status: ""
  });
  const formRef = ref();
  const dataList = ref<OAuthClient[]>([]);
  const loading = ref(true);
  const pagination = reactive<PaginationProps>({
    total: 0,
    pageSize: 10,
    currentPage: 1,
    background: true
  });

  const columns: TableColumnList = [
    { label: "Client ID", prop: "client_id", minWidth: 180 },
    { label: "名称", prop: "name", minWidth: 140 },
    {
      label: "Grant Types",
      prop: "grant_types",
      minWidth: 200,
      formatter: ({ grant_types }) => (grant_types || []).join(", ")
    },
    {
      label: "状态",
      prop: "status",
      minWidth: 90,
      cellRenderer: ({ row }) => (
        <el-tag type={row.status === "active" ? "success" : "info"}>
          {row.status === "active" ? "启用" : "禁用"}
        </el-tag>
      )
    },
    {
      label: "Access TTL",
      prop: "access_token_ttl_sec",
      minWidth: 110
    },
    {
      label: "Refresh TTL",
      prop: "refresh_token_ttl_sec",
      minWidth: 110
    },
    {
      label: "创建时间",
      prop: "created_at",
      minWidth: 170,
      formatter: ({ created_at }) =>
        created_at ? dayjs(created_at).format("YYYY-MM-DD HH:mm:ss") : "-"
    },
    {
      label: "操作",
      fixed: "right",
      width: 260,
      slot: "operation"
    }
  ];

  async function onSearch() {
    loading.value = true;
    try {
      const res = await listOAuthClients({
        keyword: form.keyword || undefined,
        status: form.status || undefined,
        page: pagination.currentPage,
        size: pagination.pageSize
      });
      dataList.value = res.list || [];
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

  function showSecretDialog(title: string, clientId: string, secret: string) {
    const { copy } = useClipboard({ source: secret });
    addDialog({
      title,
      width: "520px",
      hideFooter: false,
      closeOnClickModal: false,
      contentRenderer: () =>
        h(
          "div",
          { style: "line-height:1.8" },
          [
            h("div", { style: "color:#f56c6c; margin-bottom:8px" },
              "请立即复制保存 client_secret，关闭后将无法再次查看！"),
            h("div", null, [h("b", null, "Client ID："), clientId]),
            h(
              "div",
              { style: "word-break:break-all; margin-top:8px" },
              [h("b", null, "Client Secret："), secret]
            ),
            h(
              "el-button",
              {
                type: "primary",
                size: "small",
                style: "margin-top:12px",
                onClick: async () => {
                  await copy();
                  message("已复制到剪贴板", { type: "success" });
                }
              },
              { default: () => "复制 Secret" }
            )
          ]
        )
    });
  }

  function openDialog(title = "新增", row?: OAuthClient) {
    const isEdit = title === "修改";
    const formInline: FormItemProps = {
      client_id: row?.client_id ?? "",
      name: row?.name ?? "",
      permissions: row?.permissions ? [...row.permissions] : [],
      grant_types: row?.grant_types ? [...row.grant_types] : [],
      access_token_ttl_sec: row?.access_token_ttl_sec ?? 3600,
      refresh_token_ttl_sec: row?.refresh_token_ttl_sec ?? 2592000,
      status: row?.status ?? "active"
    };
    addDialog({
      title: `${title}令牌客户端`,
      props: {
        formInline,
        isEdit
      },
      width: "560px",
      draggable: true,
      closeOnClickModal: false,
      contentRenderer: () =>
        h(editForm, { ref: formRef, formInline: null, isEdit }),
      beforeSure: (done, { options }) => {
        const FormRef = formRef.value.getRef();
        const cur = options.props.formInline as FormItemProps;
        FormRef.validate(async valid => {
          if (!valid) return;
          try {
            if (!isEdit) {
              const res = await createOAuthClient({
                name: cur.name,
                permissions: toRaw(cur.permissions),
                grant_types: toRaw(cur.grant_types),
                access_token_ttl_sec: cur.access_token_ttl_sec,
                refresh_token_ttl_sec: cur.refresh_token_ttl_sec,
                status: cur.status
              });
              message("新增成功", { type: "success" });
              done();
              onSearch();
              showSecretDialog(
                "新建客户端成功",
                res.client_id,
                res.client_secret
              );
            } else {
              await updateOAuthClient(cur.client_id!, {
                name: cur.name,
                permissions: toRaw(cur.permissions),
                grant_types: toRaw(cur.grant_types),
                access_token_ttl_sec: cur.access_token_ttl_sec,
                refresh_token_ttl_sec: cur.refresh_token_ttl_sec,
                status: cur.status
              });
              message("修改成功", { type: "success" });
              done();
              onSearch();
            }
          } catch (e: any) {
            message(`操作失败：${e?.message || e}`, { type: "error" });
          }
        });
      }
    });
  }

  async function handleDelete(row: OAuthClient) {
    try {
      await ElMessageBox.confirm(
        `确认删除客户端 ${row.client_id} 吗？`,
        "系统提示",
        { type: "warning" }
      );
      await deleteOAuthClient(row.client_id);
      message("删除成功", { type: "success" });
      onSearch();
    } catch (e: any) {
      if (e === "cancel") return;
      message(`删除失败：${e?.message || e}`, { type: "error" });
    }
  }

  async function handleToggleStatus(row: OAuthClient) {
    const next = row.status === "active" ? "disabled" : "active";
    try {
      await updateOAuthClient(row.client_id, { status: next });
      message(`已${next === "active" ? "启用" : "禁用"}`, { type: "success" });
      onSearch();
    } catch (e: any) {
      message(`操作失败：${e?.message || e}`, { type: "error" });
    }
  }

  async function handleResetSecret(row: OAuthClient) {
    try {
      await ElMessageBox.confirm(
        `重置 ${row.client_id} 的 client_secret 后旧 secret 立即失效，确认继续？`,
        "系统提示",
        { type: "warning" }
      );
      const res = await resetOAuthClientSecret(row.client_id);
      showSecretDialog("Secret 已重置", res.client_id, res.client_secret);
    } catch (e: any) {
      if (e === "cancel") return;
      message(`重置失败：${e?.message || e}`, { type: "error" });
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
    handleToggleStatus,
    handleResetSecret,
    handleSizeChange,
    handleCurrentChange,
    handleSelectionChange
  };
}
