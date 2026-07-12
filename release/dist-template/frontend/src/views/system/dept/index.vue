<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useDept } from "./utils/hook";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";

import Delete from "~icons/ep/delete";
import EditPen from "~icons/ep/edit-pen";
import Refresh from "~icons/ep/refresh";
import AddFill from "~icons/ri/add-circle-line";
import User from "~icons/ep/user";

defineOptions({
  name: "SystemDept"
});

const { t } = useI18n();
const formRef = ref();
const tableRef = ref();
const {
  form,
  loading,
  columns,
  dataList,
  onSearch,
  resetForm,
  openDialog,
  handleDelete,
  handleAssignUsers,
  handleSelectionChange
} = useDept();

function onFullscreen() {
  tableRef.value.setAdaptive();
}
</script>

<template>
  <div class="main">
    <el-form
      ref="formRef"
      :inline="true"
      :model="form"
      class="search-form bg-bg_color w-full pl-8 pt-3 overflow-auto"
    >
      <el-form-item :label="`${t('menus.deptSearchName')}：`" prop="name">
        <el-input
          v-model="form.name"
          :placeholder="t('menus.deptPlaceholderName')"
          clearable
          class="w-45!"
        />
      </el-form-item>
      <el-form-item :label="`${t('menus.userStatus')}：`" prop="status">
        <el-select
          v-model="form.status"
          :placeholder="t('menus.deptPlaceholderStatus')"
          clearable
          class="w-45!"
        >
          <el-option :label="t('menus.deptStatusActive')" value="active" />
          <el-option :label="t('menus.deptStatusInactive')" value="inactive" />
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
      :title="t('menus.systemDept')"
      :columns="columns"
      :tableRef="tableRef?.getTableRef()"
      @refresh="onSearch"
      @fullscreen="onFullscreen"
    >
      <template #buttons>
        <el-button
          type="primary"
          :icon="useRenderIcon(AddFill)"
          @click="openDialog()"
        >
          {{ t("menus.deptNew") }}
        </el-button>
      </template>
      <template v-slot="{ size, dynamicColumns }">
        <pure-table
          ref="tableRef"
          adaptive
          :adaptiveConfig="{ offsetBottom: 45 }"
          align-whole="center"
          row-key="id"
          showOverflowTooltip
          table-layout="auto"
          default-expand-all
          :loading="loading"
          :size="size"
          :data="dataList"
          :columns="dynamicColumns"
          :header-cell-style="{
            background: 'var(--el-fill-color-light)',
            color: 'var(--el-text-color-primary)'
          }"
          @selection-change="handleSelectionChange"
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
            <el-button
              class="reset-margin"
              link
              type="primary"
              :size="size"
              :icon="useRenderIcon(User)"
              @click="handleAssignUsers(row)"
            >
              {{ t("menus.deptAssignUsers") }}
            </el-button>
            <el-button
              class="reset-margin"
              link
              type="primary"
              :size="size"
              :icon="useRenderIcon(AddFill)"
              @click="
                openDialog(t('menus.dialogActionCreate'), {
                  parentId: row.id
                } as any)
              "
            >
              {{ t("menus.notificationNew") }}
            </el-button>
            <el-popconfirm
              :title="t('menus.deptConfirmDelete', { name: row.name })"
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
          </template>
        </pure-table>
      </template>
    </PureTableBar>
  </div>
</template>

<style lang="scss" scoped>
:deep(.el-table__inner-wrapper::before) {
  height: 0;
}

.main-content {
  margin: 24px 24px 0 !important;
}

.search-form {
  :deep(.el-form-item) {
    margin-bottom: 12px;
  }
}
</style>
