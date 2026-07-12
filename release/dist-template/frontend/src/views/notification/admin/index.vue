<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useAdmin } from "./utils/hook";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import AddFill from "~icons/ri/add-circle-line";
import EditPen from "~icons/ep/edit-pen";
import VideoPlay from "~icons/ep/video-play";
import Refresh from "~icons/ep/refresh";

defineOptions({ name: "NotificationAdmin" });
const {
  form,
  loading,
  columns,
  dataList,
  onSearch,
  openDialog,
  handlePublish,
  handleRevoke
} = useAdmin();
const { t } = useI18n();
const formRef = ref();

function reset() {
  formRef.value?.resetFields();
  onSearch();
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
      <el-form-item :label="`${t('menus.notificationTitle')}：`" prop="keyword">
        <el-input
          v-model="form.keyword"
          :placeholder="t('menus.notificationFilterTitle')"
          clearable
          class="w-50!"
        />
      </el-form-item>
      <el-form-item :label="`${t('menus.notificationStatus')}：`" prop="status">
        <el-select
          v-model="form.status"
          :placeholder="t('menus.notificationFilterAll')"
          clearable
          class="w-40!"
        >
          <el-option :label="t('menus.notificationDraft')" value="draft" />
          <el-option
            :label="t('menus.notificationPublished')"
            value="published"
          />
          <el-option :label="t('menus.notificationRevoked')" value="revoked" />
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
        <el-button :icon="useRenderIcon(Refresh)" @click="reset">
          {{ t("menus.notificationReset") }}
        </el-button>
      </el-form-item>
    </el-form>

    <PureTableBar :title="t('menus.notificationAdmin')" @refresh="onSearch">
      <template #buttons>
        <el-button
          type="primary"
          :icon="useRenderIcon(AddFill)"
          @click="openDialog()"
        >
          {{ t("menus.notificationNew") }}
        </el-button>
      </template>
      <template v-slot="{ size, dynamicColumns }">
        <pure-table
          align-whole="center"
          :loading="loading"
          :size="size"
          :data="dataList"
          :columns="dynamicColumns"
        >
          <template #operation="{ row }">
            <el-button
              link
              type="primary"
              :icon="useRenderIcon(EditPen)"
              @click="openDialog(row)"
            >
              {{ t("menus.notificationEdit") }}
            </el-button>
            <el-button
              link
              type="success"
              :icon="useRenderIcon(VideoPlay)"
              @click="handlePublish(row)"
            >
              {{ t("menus.notificationPublish") }}
            </el-button>
            <el-button link type="danger" @click="handleRevoke(row)">
              {{ t("menus.notificationRevoke") }}
            </el-button>
          </template>
        </pure-table>
      </template>
    </PureTableBar>
  </div>
</template>
