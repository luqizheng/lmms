<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useTokenClient } from "./utils/hook";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";

import Delete from "~icons/ep/delete";
import EditPen from "~icons/ep/edit-pen";
import Refresh from "~icons/ep/refresh";
import Key from "~icons/ep/key";
import AddFill from "~icons/ri/add-circle-line";

defineOptions({
  name: "SystemTokenClient"
});

const { t } = useI18n();
const formRef = ref();

const {
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
} = useTokenClient();
</script>

<template>
  <div class="main">
    <el-form
      ref="formRef"
      :inline="true"
      :model="form"
      class="search-form bg-bg_color w-full pl-8 pt-3 overflow-auto"
    >
      <el-form-item :label="`${t('menus.tokenClientSearchKeyword')}：`" prop="keyword">
        <el-input
          v-model="form.keyword"
          :placeholder="t('menus.tokenClientPlaceholderKeyword')"
          clearable
          class="w-50!"
        />
      </el-form-item>
      <el-form-item :label="`${t('menus.userStatus')}：`" prop="status">
        <el-select
          v-model="form.status"
          :placeholder="t('menus.notificationFilterAll')"
          clearable
          class="w-40!"
        >
          <el-option :label="t('menus.deptStatusActive')" value="active" />
          <el-option :label="t('menus.tokenClientStatusDisabled')" value="disabled" />
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

    <PureTableBar :title="t('menus.systemTokenClient')" :columns="columns" @refresh="onSearch">
      <template #buttons>
        <el-button
          type="primary"
          :icon="useRenderIcon(AddFill)"
          @click="openDialog()"
        >
          {{ t("menus.tokenClientNew") }}
        </el-button>
      </template>
      <template v-slot="{ size, dynamicColumns }">
        <pure-table
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
              @click="openDialog('修改', row)"
            >
              {{ t("menus.userEdit") }}
            </el-button>
            <el-button
              class="reset-margin"
              link
              type="primary"
              :size="size"
              @click="handleToggleStatus(row)"
            >
              {{
                row.status === "active"
                  ? t("menus.tokenClientDisable")
                  : t("menus.tokenClientEnable")
              }}
            </el-button>
            <el-button
              class="reset-margin"
              link
              type="warning"
              :size="size"
              :icon="useRenderIcon(Key)"
              @click="handleResetSecret(row)"
            >
              {{ t("menus.tokenClientResetSecret") }}
            </el-button>
            <el-popconfirm
              :title="t('menus.tokenClientConfirmDelete', { clientId: row.client_id })"
              @confirm="handleDelete(row)"
            >
              <template #reference>
                <el-button
                  class="reset-margin"
                  link
                  type="danger"
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
.search-form {
  :deep(.el-form-item) {
    margin-bottom: 12px;
  }
}
</style>