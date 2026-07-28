<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useI18n } from "vue-i18n";
import { useInbox } from "./utils/hook";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { markRead } from "@/api/notification";
import { useNotificationStore } from "@/store/modules/notification";
import dayjs from "dayjs";
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

const drawerVisible = ref(false);
const drawerItem = ref<any>(null);
const drawerLoading = ref(false);

async function openDrawer(row: any) {
  drawerItem.value = row;
  drawerVisible.value = true;
  // 进入即标已读
  try {
    drawerLoading.value = true;
    await markRead(row.id);
    useNotificationStore().readOne(row.id);
  } catch {
    // ignore
  } finally {
    drawerLoading.value = false;
  }
}

function closeDrawer() {
  drawerVisible.value = false;
  drawerItem.value = null;
}

function gotoDetail() {
  if (!drawerItem.value) return;
  closeDrawer();
  openDetail(drawerItem.value);
}

function reset() {
  formRef.value?.resetFields();
  onSearch();
}

async function markAllRead() {
  if (!unreadCount.value) return;
  marking.value = true;
  try {
    const { useNotificationStore } =
      await import("@/store/modules/notification");
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

function fmtDateTime(ts: string | number | null): string {
  if (!ts) return "—";
  return dayjs(ts).format("YYYY-MM-DD HH:mm");
}

onMounted(() => {
  // 首次进入时请求浏览器通知权限（用于后续 Notification API 弹桌面通知）
  if (
    typeof Notification !== "undefined" &&
    Notification.permission === "default"
  ) {
    Notification.requestPermission().catch(() => {
      // 用户拒绝不影响功能
    });
  }
});
</script>

<template>
  <div class="main">
    <el-form
      ref="formRef"
      :inline="true"
      :model="form"
      class="search-form pl-8 pt-3"
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
    <PureTableBar :title="t('menus.notificationInbox')" :columns="columns" @refresh="onSearch">
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
          @row-click="openDrawer"
        />
      </template>
    </PureTableBar>

    <el-drawer
      v-model="drawerVisible"
      direction="rtl"
      size="50%"
      :with-header="false"
      :destroy-on-close="false"
    >
      <div v-if="drawerItem" class="drawer-content">
        <div class="drawer-header">
          <h2 class="drawer-title">{{ drawerItem.title }}</h2>
          <div class="drawer-meta">
            <el-tag v-if="drawerItem.pinned" type="warning" size="small">
              {{ t("menus.detailPinnedTag") }}
            </el-tag>
            <el-tag v-if="drawerItem.popover" type="info" size="small">
              {{ t("menus.detailPopoverTag") }}
            </el-tag>
            <span class="text-gray-400 text-xs ml-2">
              {{ t("menus.detailPublishLabel")
              }}{{ fmtDateTime(drawerItem.publish_at) }}
            </span>
            <span
              v-if="drawerItem.expire_at"
              class="text-gray-400 text-xs ml-2"
            >
              {{ t("menus.detailExpireLabel")
              }}{{ fmtDateTime(drawerItem.expire_at) }}
            </span>
          </div>
        </div>
        <div class="drawer-body">
          <div v-if="drawerLoading" v-loading="true" class="h-32" />
          <pre v-else class="drawer-body-text">{{ drawerItem.body }}</pre>
        </div>
        <div class="drawer-footer">
          <el-button @click="closeDrawer">
            {{ t("menus.inboxDetailClose") }}
          </el-button>
          <el-button type="primary" @click="gotoDetail">
            {{ t("menus.inboxMarkRead") }} / Detail
          </el-button>
        </div>
      </div>
    </el-drawer>
  </div>
</template>

<style lang="scss" scoped>
.drawer-content {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 24px;
}

.drawer-header {
  border-bottom: 1px solid var(--el-border-color-lighter);
  padding-bottom: 16px;
}

.drawer-title {
  font-size: 20px;
  font-weight: 600;
  margin: 0 0 8px;
  color: var(--el-text-color-primary);
}

.drawer-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.drawer-body {
  flex: 1;
  overflow-y: auto;
  padding: 16px 0;
}

.drawer-body-text {
  white-space: pre-wrap;
  word-wrap: break-word;
  font-family: inherit;
  font-size: 14px;
  line-height: 1.6;
  color: var(--el-text-color-regular);
  margin: 0;
}

.drawer-footer {
  border-top: 1px solid var(--el-border-color-lighter);
  padding-top: 16px;
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
