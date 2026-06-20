<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import { useRouter } from "vue-router";
import { Bell } from "@element-plus/icons-vue";
import { useNotificationStore } from "@/store/modules/notification";
import { connect, disconnect, on } from "@/utils/notification-socket";

const router = useRouter();
const store = useNotificationStore();

let offAnnouncement: (() => void) | undefined;
let offCount: (() => void) | undefined;

onMounted(() => {
  store.refreshUnread().catch(() => undefined);
  connect();
  offAnnouncement = on("announcement", (msg: any) => {
    if (typeof msg.id === "number" && typeof msg.title === "string") {
      store.pushFromSocket({ id: msg.id, title: msg.title });
    }
  });
  offCount = on("unread_count", (msg: any) => {
    if (typeof msg.count === "number") store.unreadCount = msg.count;
  });
});

onBeforeUnmount(() => {
  offAnnouncement?.();
  offCount?.();
  disconnect();
});

function goInbox() {
  router.push("/notification/inbox");
}
</script>

<template>
  <el-badge
    :value="store.unreadCount"
    :hidden="store.unreadCount === 0"
    :max="99"
  >
    <el-button text :icon="Bell" @click="goInbox" />
  </el-badge>
</template>
