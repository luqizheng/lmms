import editForm from "../form.vue";
import { handleTree } from "@/utils/tree";
import { message } from "@/utils/message";
import { transformI18n } from "@/plugins/i18n";
import { addDialog } from "@/components/ReDialog";
import { reactive, ref, onMounted, h, computed } from "vue";
import type {
  FormItemProps,
  MenuConfigSummary,
  RoleOption
} from "../utils/types";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { cloneDeep, isAllEmpty, deviceDetection } from "@pureadmin/utils";
import { listIamRoles } from "@/api/iam";
import { constantMenus } from "@/router";
import { buildStaticMenuSeed } from "@/router/menu-merge";
import {
  listMenuConfigs,
  createMenuConfig,
  getMenuConfig,
  updateMenuConfig,
  publishMenuConfig,
  cloneMenuConfig
} from "@/api/menu-config";
import { formItemsToRoutes, routesToFormItems } from "./menu-adapter";

export function useMenu() {
  const form = reactive({
    title: ""
  });

  const formRef = ref();
  const dataList = ref([]);
  const loading = ref(true);
  const menuConfigs = ref<MenuConfigSummary[]>([]);
  const selectedConfigId = ref("");
  const currentConfig = ref<MenuConfigSummary | null>(null);
  const flatMenuList = ref<FormItemProps[]>([]);
  const roleOptions = ref<RoleOption[]>([]);

  const isEditable = computed(() => currentConfig.value?.status === "draft");

  async function loadRoleOptions() {
    try {
      const resp = await listIamRoles({ page: 1, size: 200 });
      roleOptions.value = (resp?.items ?? [])
        .filter((r: any) => !r.status || r.status === "active")
        .map((r: any) => ({ label: r.name, value: r.code }));
    } catch {
      roleOptions.value = [];
    }
  }

  const getMenuType = (type, text = false) => {
    switch (type) {
      case 0:
        return text ? "菜单" : "primary";
      case 1:
        return text ? "iframe" : "warning";
      case 2:
        return text ? "外链" : "danger";
      case 3:
        return text ? "按钮" : "info";
    }
  };

  const columns: TableColumnList = [
    {
      label: "菜单名称",
      prop: "title",
      align: "left",
      cellRenderer: ({ row }) => (
        <>
          <span class="inline-block mr-1">
            {h(useRenderIcon(row.icon), {
              style: { paddingTop: "1px" }
            })}
          </span>
          <span>{transformI18n(row.title)}</span>
        </>
      )
    },
    {
      label: "菜单类型",
      prop: "menuType",
      width: 100,
      cellRenderer: ({ row, props }) => (
        <el-tag
          size={props.size}
          type={getMenuType(row.menuType) as any}
          effect="plain"
        >
          {getMenuType(row.menuType, true)}
        </el-tag>
      )
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
      label: "权限标识",
      prop: "auths"
    },
    {
      label: "可见角色",
      prop: "roles",
      minWidth: 140,
      formatter: ({ roles }) =>
        !roles || roles.length === 0 ? "所有人" : roles.join(",")
    },
    {
      label: "排序",
      prop: "rank",
      width: 100
    },
    {
      label: "隐藏",
      prop: "showLink",
      formatter: ({ showLink }) => (showLink ? "否" : "是"),
      width: 100
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

  async function loadConfigList() {
    const { code, data } = await listMenuConfigs();
    if (code !== 0) {
      menuConfigs.value = [];
      return;
    }

    menuConfigs.value = data.items;
    const preferred =
      data.items.find(item => item.status === "draft") ??
      data.items.find(item => item.is_active) ??
      data.items[0];

    if (preferred) {
      selectedConfigId.value = preferred.id;
      await loadSelectedConfig(preferred.id);
      return;
    }

    // 首次使用：用前端静态菜单生成种子草稿（demo 异常页/引导页默认隐藏），
    // 管理员在此基础上调整可见性/角色后发布
    const seed = buildStaticMenuSeed(constantMenus as any[]);
    const created = await createMenuConfig({
      name: `菜单草稿 ${new Date().toLocaleString()}`,
      description: "由静态菜单初始化",
      routes_json: seed
    });
    selectedConfigId.value = created.data.id;
    await loadConfigList();
  }

  async function loadSelectedConfig(id = selectedConfigId.value) {
    if (!id) return;
    loading.value = true;
    try {
      const { code, data } = await getMenuConfig(id);
      if (code !== 0) return;
      currentConfig.value = data;
      selectedConfigId.value = data.id;
      flatMenuList.value = routesToFormItems(data.routes_json as any[]);
      onSearch();
    } finally {
      loading.value = false;
    }
  }

  async function onSearch() {
    loading.value = true;
    let newData = [...flatMenuList.value];
    if (!isAllEmpty(form.title)) {
      newData = newData.filter(item =>
        transformI18n(item.title).includes(form.title)
      );
    }
    dataList.value = handleTree(newData);
    setTimeout(() => {
      loading.value = false;
    }, 200);
  }

  function nextMenuId() {
    const maxId = flatMenuList.value.reduce(
      (max, item) => Math.max(max, Number(item.id ?? 0)),
      0
    );
    return maxId + 1;
  }

  async function saveCurrentMenus() {
    if (!selectedConfigId.value || !isEditable.value) {
      message("只允许编辑草稿版本", { type: "warning" });
      return false;
    }

    const etag = currentConfig.value?.routes_etag;
    if (!etag) {
      message("菜单版本状态缺失，请重新打开菜单管理", { type: "error" });
      return false;
    }

    const routes_json = formItemsToRoutes(flatMenuList.value);
    try {
      const {
        code,
        data,
        message: msg
      } = await updateMenuConfig(selectedConfigId.value, {
        routes_json,
        if_match_etag: etag
      });

      if (code !== 0) {
        message(msg || "保存菜单失败", { type: "error" });
        return false;
      }

      currentConfig.value = data;
      flatMenuList.value = routesToFormItems(data.routes_json as any[]);
      await onSearch();
      return true;
    } catch (err: any) {
      const status = err?.response?.status;
      if (status === 409) {
        message("菜单草稿已被他人修改，已加载最新版本，请重新编辑", {
          type: "warning"
        });
        await loadSelectedConfig(selectedConfigId.value);
        return false;
      }
      const serverMsg = err?.response?.data?.message;
      message(serverMsg || "保存菜单失败", { type: "error" });
      return false;
    }
  }

  function removeMenuAndChildren(id: number) {
    const removing = new Set<number>([id]);
    let changed = true;
    while (changed) {
      changed = false;
      for (const item of flatMenuList.value) {
        const itemId = Number(item.id ?? 0);
        if (!removing.has(itemId) && removing.has(Number(item.parentId ?? 0))) {
          removing.add(itemId);
          changed = true;
        }
      }
    }
    flatMenuList.value = flatMenuList.value.filter(
      item => !removing.has(Number(item.id ?? 0))
    );
  }

  function formatHigherMenuOptions(treeList) {
    if (!treeList || !treeList.length) return;
    const newTreeList = [];
    for (let i = 0; i < treeList.length; i++) {
      treeList[i].title = transformI18n(treeList[i].title);
      formatHigherMenuOptions(treeList[i].children);
      newTreeList.push(treeList[i]);
    }
    return newTreeList;
  }

  function openDialog(title = "新增", row?: FormItemProps) {
    addDialog({
      title: `${title}菜单`,
      props: {
        roleOptions: roleOptions.value,
        formInline: {
          menuType: row?.menuType ?? 0,
          higherMenuOptions: formatHigherMenuOptions(cloneDeep(dataList.value)),
          parentId: row?.parentId ?? 0,
          title: row?.title ?? "",
          name: row?.name ?? "",
          path: row?.path ?? "",
          component: row?.component ?? "",
          rank: row?.rank ?? 99,
          redirect: row?.redirect ?? "",
          icon: row?.icon ?? "",
          extraIcon: row?.extraIcon ?? "",
          enterTransition: row?.enterTransition ?? "",
          leaveTransition: row?.leaveTransition ?? "",
          activePath: row?.activePath ?? "",
          auths: row?.auths ?? "",
          roles: row?.roles ? [...row.roles] : [],
          frameSrc: row?.frameSrc ?? "",
          frameLoading: row?.frameLoading ?? true,
          keepAlive: row?.keepAlive ?? false,
          hiddenTag: row?.hiddenTag ?? false,
          fixedTag: row?.fixedTag ?? false,
          showLink: row?.showLink ?? true,
          showParent: row?.showParent ?? false
        }
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
        async function chores() {
          if (!isEditable.value) {
            message("只允许编辑草稿版本", { type: "warning" });
            return;
          }

          if (title === "新增") {
            flatMenuList.value.push({
              ...curData,
              id: nextMenuId(),
              higherMenuOptions: []
            });
          } else {
            const index = flatMenuList.value.findIndex(
              item => item.id === row?.id
            );
            if (index >= 0) {
              flatMenuList.value[index] = {
                ...flatMenuList.value[index],
                ...curData,
                id: row?.id,
                higherMenuOptions: []
              };
            }
          }

          const saved = await saveCurrentMenus();
          if (!saved) return;

          message(
            `您${title}了菜单名称为${transformI18n(curData.title)}的这条数据`,
            { type: "success" }
          );
          done();
        }
        FormRef.validate(valid => {
          if (valid) {
            chores();
          }
        });
      }
    });
  }

  async function handleDelete(row) {
    if (!isEditable.value) {
      message("只允许编辑草稿版本", { type: "warning" });
      return;
    }
    removeMenuAndChildren(Number(row.id));
    const saved = await saveCurrentMenus();
    if (!saved) return;
    message(`您删除了菜单名称为${transformI18n(row.title)}的这条数据`, {
      type: "success"
    });
  }

  async function createDraftFromCurrent() {
    if (selectedConfigId.value) {
      const {
        code,
        data,
        message: msg
      } = await cloneMenuConfig(selectedConfigId.value);
      if (code !== 0) {
        message(msg || "新建草稿失败", { type: "error" });
        return;
      }
      selectedConfigId.value = data.id;
      await loadConfigList();
      await loadSelectedConfig(data.id);
      return;
    }

    const created = await createMenuConfig({
      name: `菜单草稿 ${new Date().toLocaleString()}`,
      description: "前端菜单管理创建",
      routes_json: []
    });
    selectedConfigId.value = created.data.id;
    await loadConfigList();
    await loadSelectedConfig(created.data.id);
  }

  async function publishCurrentConfig() {
    if (!selectedConfigId.value || !isEditable.value) return;
    const {
      code,
      data,
      message: msg
    } = await publishMenuConfig(selectedConfigId.value);
    if (code !== 0) {
      message(msg || "发布菜单版本失败", { type: "error" });
      return;
    }
    message("发布成功", { type: "success" });
    currentConfig.value = data;
    await loadConfigList();
    await loadSelectedConfig(data.id);
  }

  onMounted(() => {
    loadRoleOptions();
    loadConfigList();
  });

  return {
    form,
    loading,
    columns,
    dataList,
    menuConfigs,
    selectedConfigId,
    currentConfig,
    isEditable,
    roleOptions,
    onSearch,
    resetForm,
    openDialog,
    handleDelete,
    handleSelectionChange,
    loadSelectedConfig,
    createDraftFromCurrent,
    publishCurrentConfig
  };
}
