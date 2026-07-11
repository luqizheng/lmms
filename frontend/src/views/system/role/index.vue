<script setup lang="ts">
import { useRole } from "./utils/hook";
import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { deviceDetection } from "@pureadmin/utils";

import Delete from "~icons/ep/delete";
import EditPen from "~icons/ep/edit-pen";
import Refresh from "~icons/ep/refresh";
import Menu from "~icons/ep/menu";
import AddFill from "~icons/ri/add-circle-line";

defineOptions({
  name: "SystemRole"
});

const { t } = useI18n();

const _iconClass = computed(() => {
  return [
    "size-5.5",
    "flex-c",
    "outline-hidden",
    "rounded-sm",
    "cursor-pointer",
    "transition-colors"
  ];
});

const treeRef = ref();
const formRef = ref();
const tableRef = ref();

const {
  form,
  loading,
  columns,
  dataList,
  pagination,
  permissionDialogVisible,
  groupedPermissions,
  checkedPermissionIds,
  curRow,
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
} = useRole(treeRef);
</script>

<template>
  <div class="main">
    <el-form
      ref="formRef"
      :inline="true"
      :model="form"
      class="search-form bg-bg_color w-full pl-8 pt-3 overflow-auto"
    >
      <el-form-item :label="`${t('menus.roleSearchName')}：`" prop="name">
        <el-input
          v-model="form.name"
          :placeholder="t('menus.rolePlaceholderName')"
          clearable
          class="w-45!"
        />
      </el-form-item>
      <el-form-item :label="`${t('menus.roleSearchCode')}：`" prop="code">
        <el-input
          v-model="form.code"
          :placeholder="t('menus.rolePlaceholderCode')"
          clearable
          class="w-45!"
        />
      </el-form-item>
      <el-form-item :label="`${t('menus.userStatus')}：`" prop="status">
        <el-select
          v-model="form.status"
          :placeholder="t('menus.rolePlaceholderStatus')"
          clearable
          class="w-45!"
        >
          <el-option :label="t('menus.roleStatusActive')" value="active" />
          <el-option :label="t('menus.roleStatusInactive')" value="disabled" />
        </el-select>
      </el-form-item>
      <el-form-item>
        <el-button
          type="primary"
          :icon="useRenderIcon('ri/search-line')"
          :loading="loading"
          @click="onSearch"
        >
          {{ t("menus.notificationSearch") }}
        </el-button>
        <el-button :icon="useRenderIcon(Refresh)" @click="resetForm(formRef)">
          {{ t("menus.notificationReset") }}
        </el-button>
      </el-form-item>
    </el-form>

    <PureTableBar
      :title="t('menus.systemRole')"
      :columns="columns"
      @refresh="onSearch"
    >
      <template #buttons>
        <el-button
          type="primary"
          :icon="useRenderIcon(AddFill)"
          @click="openDialog()"
        >
          {{ t("menus.roleNew") }}
        </el-button>
      </template>
      <template v-slot="{ size, dynamicColumns }">
        <pure-table
          ref="tableRef"
          align-whole="center"
          showOverflowTooltip
          table-layout="auto"
          :loading="loading"
          :size="size"
          adaptive
          :adaptiveConfig="{ offsetBottom: 108 }"
          :data="dataList"
          :columns="dynamicColumns"
          :pagination="{ ...pagination, size }"
          :header-cell-style="{
            background: 'var(--el-fill-color-light)',
            color: 'var(--el-text-color-primary)'
          }"
          @selection-change="handleSelectionChange"
          @page-size-change="handleSizeChange"
          @page-current-change="handleCurrentChange"
        >
          <template #operation="{ row }">
            <el-button
              class="reset-margin"
              link
              type="primary"
              :size="size"
              :icon="useRenderIcon(EditPen)"
              @click="openDialog(t('menus.dialogActionEdit'), row)"
            >
              {{ t("menus.userEdit") }}
            </el-button>
            <el-popconfirm
              :title="t('menus.roleConfirmDelete', { name: row.name })"
              @confirm="handleDelete(row)"
            >
              <template #reference>
                <el-button
                  class="reset-margin"
                  link
                  type="primary"
                  :size="size"
                  :icon="useRenderIcon(Delete)"
                >
                  {{ t("menus.userDelete") }}
                </el-button>
              </template>
            </el-popconfirm>
            <el-button
              class="reset-margin"
              link
              type="primary"
              :size="size"
              :icon="useRenderIcon(Menu)"
              @click="handleMenu(row)"
            >
              {{ t("menus.rolePermissionBtn") }}
            </el-button>
          </template>
        </pure-table>
      </template>
    </PureTableBar>

    <el-dialog
      v-model="permissionDialogVisible"
      :title="
        curRow?.name
          ? t('menus.roleAssignPermissionTitle', { name: curRow.name })
          : t('menus.roleAssignPermissionTitleEmpty')
      "
      width="600px"
      :fullscreen="deviceDetection()"
      :close-on-click-modal="false"
      @close="closePermissionDialog"
    >
      <div class="permission-wrap">
        <el-empty
          v-if="Object.keys(groupedPermissions).length === 0"
          :description="t('menus.roleNoPermission')"
        />
        <div
          v-for="(items, category) in groupedPermissions"
          :key="category"
          class="mb-4"
        >
          <div class="font-bold mb-2 text-(--el-text-color-primary)">
            {{ category }}
          </div>
          <el-checkbox-group v-model="checkedPermissionIds">
            <el-checkbox
              v-for="p in items"
              :key="p.id"
              :value="p.id"
              :label="p.name"
            >
              {{ p.name }}（{{ p.code }}）
            </el-checkbox>
          </el-checkbox-group>
        </div>
      </div>
      <template #footer>
        <el-button @click="closePermissionDialog">
          {{ t("menus.roleCancel") }}
        </el-button>
        <el-button type="primary" @click="handleSave">
          {{ t("menus.profileConfirm") }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style lang="scss" scoped>
:deep(.el-dropdown-menu__item i) {
  margin: 0;
}

.main-content {
  margin: 24px 24px 0 !important;
}

.search-form {
  :deep(.el-form-item) {
    margin-bottom: 12px;
  }
}

.permission-wrap {
  max-height: 60vh;
  overflow-y: auto;
}
</style>
