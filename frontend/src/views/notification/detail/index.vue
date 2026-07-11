<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import dayjs from "dayjs";
import { getAnnouncement, markRead } from "@/api/notification";
import { useNotificationStore } from "@/store/modules/notification";

defineOptions({ name: "NotificationDetail" });
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const store = useNotificationStore();
const data = ref<any>(null);

onMounted(async () => {
  const id = Number(route.params.id);
  try {
    data.value = await getAnnouncement(id);
    try {
      await markRead(id);
      store.readOne(id);
    } catch {
      // ignore
    }
  } catch (e: any) {
    data.value = {
      title: t("menus.detailLoadFail"),
      body: e?.message ?? String(e)
    };
  }
});
</script>

<template>
  <div class="main p-6">
    <el-page-header @back="router.back()">
      <template #content>
        <span class="text-lg font-medium">{{
          data?.title ?? t("menus.detailLoading")
        }}</span>
      </template>
    </el-page-header>
    <div v-if="data" class="mt-6 bg-bg_color p-6 rounded">
      <div class="text-sm text-gray-500 mb-4">
        {{ t("menus.detailPublishLabel")
        }}{{
          data.publish_at
            ? dayjs(data.publish_at).format("YYYY-MM-DD HH:mm")
            : "-"
        }}
        <span v-if="data.expire_at">
          {{ t("menus.detailExpireLabel")
          }}{{ dayjs(data.expire_at).format("YYYY-MM-DD HH:mm") }}
        </span>
        <el-tag v-if="data.pinned" class="ml-2" type="warning">
          {{ t("menus.detailPinnedTag") }}
        </el-tag>
        <el-tag v-if="data.popover" class="ml-2" type="info">
          {{ t("menus.detailPopoverTag") }}
        </el-tag>
      </div>
      <div class="whitespace-pre-wrap">{{ data.body }}</div>
    </div>
  </div>
</template>
