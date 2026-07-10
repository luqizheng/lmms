<script setup lang="ts">
import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import { useInbox } from "./utils/hook";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import Refresh from "~icons/ep/refresh";
import Search from "~icons/ri/search-line";

defineOptions({ name: "NotificationInbox" });
const { t } = useI18n();
const formRef = ref();
const marking = ref(false);
const {
  form,
  loading,
  columns,
  dataList,
  activeTab,
  unreadCount,
  onSearch,
  onTabChange,
  openDetail
} = useInbox();

function reset() {
  formRef.value?.resetFields();
  onSearch();
}

async function markAllRead() {
  if (!unreadCount.value) return;
  marking.value = true;
  try {
    // 复用 store.readAll（按 popoverQueue 全量 mark read + 清空）
    const { useNotificationStore } = await import(
      "@/store/modules/notification"
    );
    await useNotificationStore().readAll();
    await onSearch();
  } finally {
    marking.value = false;
  }
}

const tabCounts = computed(() => ({
  all: dataList.value.length,
  unread: unreadCount.value,
  read: dataList.value.length - unreadCount.value
}));
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
      <el-form-item>
        <el-button
          type="primary"
          :icon="useRenderIcon(Search)"
          :loading="loading"
          @click="onSearch"
        >
          {{ t("menus.notificationSearch") }}
        </el-button>
        <el-button :icon="useRenderIcon(Refresh)" @click="reset">
          {{ t("menus.notificationReset") }}
        </el-button>
        <el-button
          v-if="unreadCount > 0"
          type="primary"
          :loading="marking"
          @click="markAllRead"
        >
          {{ t("menus.notificationMarkAllRead") }}
        </el-button>
      </el-form-item>
    </el-form>
    <PureTableBar :title="t('menus.notificationInbox')" @refresh="onSearch">
      <template v-slot="{ size, dynamicColumns }">
        <el-tabs v-model="activeTab" class="ml-4" @tab-change="onTabChange">
          <el-tab-pane :name="'all'" :label="t('menus.notificationAll')" />
          <el-tab-pane
            :name="'unread'"
            :label="`${t('menus.notificationUnread')} (${tabCounts.unread})`"
          />
          <el-tab-pane
            :name="'read'"
            :label="`${t('menus.notificationRead')} (${tabCounts.read})`"
          />
        </el-tabs>
        <pure-table
          align-whole="center"
          :loading="loading"
          :size="size"
          :data="dataList"
          :columns="dynamicColumns"
          @row-click="openDetail"
        />
      </template>
    </PureTableBar>
  </div>
</template>