<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useMenu } from "./utils/hook";
import { transformI18n } from "@/plugins/i18n";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";

import Delete from "~icons/ep/delete";
import EditPen from "~icons/ep/edit-pen";
import Refresh from "~icons/ep/refresh";
import AddFill from "~icons/ri/add-circle-line";

defineOptions({
  name: "SystemMenu"
});

const { t } = useI18n();

const formRef = ref();
const tableRef = ref();
const {
  form,
  loading,
  columns,
  dataList,
  menuConfigs,
  selectedConfigId,
  currentConfig,
  isEditable,
  onSearch,
  resetForm,
  openDialog,
  handleDelete,
  handleSelectionChange,
  loadSelectedConfig,
  createDraftFromCurrent,
  publishCurrentConfig
} = useMenu();

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
      <el-form-item :label="`${t('menus.menuColumnTitle')}：`" prop="title">
        <el-input
          v-model="form.title"
          :placeholder="t('menus.menuPlaceholderTitle')"
          clearable
          class="w-45!"
        />
      </el-form-item>
      <el-form-item :label="`${t('menus.menuColumnVersion')}：`">
        <el-select
          v-model="selectedConfigId"
          class="w-70!"
          :placeholder="t('menus.menuPlaceholderVersion')"
          @change="loadSelectedConfig"
        >
          <el-option
            v-for="item in menuConfigs"
            :key="item.id"
            :label="t('menus.menuVersionOptionFormat', {
              version: item.version,
              name: item.name,
              status: item.status,
              active: item.is_active ? t('menus.menuVersionActive') : t('menus.menuVersionNotActive')
            })"
            :value="item.id"
          />
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
      :title="currentConfig
        ? `${t('menus.systemMenu')}（v${currentConfig.version} ${currentConfig.status}）`
        : t('menus.systemMenu')"
      :columns="columns"
      :isExpandAll="false"
      :tableRef="tableRef?.getTableRef()"
      @refresh="onSearch"
      @fullscreen="onFullscreen"
    >
      <template #buttons>
        <el-button :loading="loading" @click="createDraftFromCurrent">
          {{ t("menus.menuNewDraft") }}
        </el-button>
        <el-button
          v-if="isEditable"
          type="success"
          :loading="loading"
          @click="publishCurrentConfig"
        >
          {{ t("menus.menuPublishVersion") }}
        </el-button>
        <el-button
          type="primary"
          :icon="useRenderIcon(AddFill)"
          :disabled="!isEditable"
          @click="openDialog()"
        >
          {{ t("menus.menuNewMenu") }}
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
              :disabled="!isEditable"
              @click="openDialog('修改', row)"
            >
              {{ t("menus.userEdit") }}
            </el-button>
            <el-button
              v-show="row.menuType !== 3"
              class="reset-margin"
              link
              type="primary"
              :size="size"
              :icon="useRenderIcon(AddFill)"
              :disabled="!isEditable"
              @click="openDialog('新增', { parentId: row.id } as any)"
            >
              {{ t("menus.notificationNew") }}
            </el-button>
            <el-popconfirm
              :title="t('menus.menuConfirmDelete', { name: transformI18n(row.title) }) + (row?.children?.length > 0 ? '。' + t('menus.menuChildrenNote') : '')"
              @confirm="handleDelete(row)"
            >
              <template #reference>
                <el-button
                  class="reset-margin"
                  link
                  type="primary"
                  :size="size"
                  :icon="useRenderIcon(Delete)"
                  :disabled="!isEditable"
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