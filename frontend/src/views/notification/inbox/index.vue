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