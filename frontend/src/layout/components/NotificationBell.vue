<script setup lang="ts">
import { computed, ref, watch, onBeforeUnmount, onMounted } from "vue";
import { useRouter } from "vue-router";
import { Bell } from "@element-plus/icons-vue";
import { useI18n } from "vue-i18n";
import {
  useNotificationStore,
  type NotificationItem
} from "@/store/modules/notification";
import { connect, disconnect, on } from "@/utils/notification-socket";

const router = useRouter();
const store = useNotificationStore();
const { t } = useI18n();

let offAnnouncement: (() => void) | undefined;
let offCount: (() => void) | undefined;

const POPOVER_LIMIT = 5;
const POPOVER_RECENT_FETCH = 20;

const pulse = ref(false);
let pulseTimer: ReturnType<typeof setTimeout> | null = null;
const lastQueueLen = ref(0);

// ============ 桌面通知 + audio cue ============
let audioCtx: AudioContext | null = null;

function getAudioContext(): AudioContext | null {
  if (typeof window === "undefined") return null;
  if (audioCtx) return audioCtx;
  const Ctor =
    (window as any).AudioContext || (window as any).webkitAudioContext;
  if (!Ctor) return null;
  try {
    audioCtx = new Ctor();
  } catch {
    return null;
  }
  return audioCtx;
}

function playBeep() {
  const ctx = getAudioContext();
  if (!ctx) return;
  // 浏览器策略要求用户首次交互后才能 resume 音频 context
  if (ctx.state === "suspended") {
    ctx.resume().catch(() => undefined);
  }
  try {
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.frequency.value = 880;
    osc.type = "sine";
    gain.gain.setValueAtTime(0.15, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.25);
    osc.connect(gain).connect(ctx.destination);
    osc.start();
    osc.stop(ctx.currentTime + 0.3);
  } catch {
    // 忽略音频播放失败
  }
}

let lastDesktopNotif: Notification | null = null;

function fireDesktopNotification(title: string) {
  if (
    typeof window === "undefined" ||
    typeof Notification === "undefined"
  ) {
    return;
  }
  if (Notification.permission !== "granted") return;
  try {
    if (lastDesktopNotif) lastDesktopNotif.close();
    lastDesktopNotif = new Notification(
      t("menus.notificationDesktopTitle"),
      {
        body: t("menus.notificationDesktopBody", { title }),
        tag: "notification-bell",
        silent: false
      }
    );
    lastDesktopNotif.onclick = () => {
      window.focus();
      router.push("/notification/inbox");
      lastDesktopNotif?.close();
      lastDesktopNotif = null;
    };
    setTimeout(() => {
      lastDesktopNotif?.close();
      lastDesktopNotif = null;
    }, 8000);
  } catch {
    // 忽略
  }
}

onMounted(() => {
  store.refreshUnread().catch(() => undefined);
  store.refreshInbox().catch(() => undefined);
  connect();
  lastQueueLen.value = store.popoverQueue.length;
  // 首次进入时请求浏览器通知权限（弹桌面通知前需 granted）
  if (
    typeof Notification !== "undefined" &&
    Notification.permission === "default"
  ) {
    Notification.requestPermission().catch(() => undefined);
  }
  offAnnouncement = on("announcement", (msg: any) => {
    if (typeof msg.id !== "number" || typeof msg.title !== "string") return;
    store.pushFromSocket({ id: msg.id, title: msg.title });
    // WS 推送触发：弹桌面通知 + audio cue
    fireDesktopNotification(msg.title);
    playBeep();
  });
  offCount = on("unread_count", (msg: any) => {
    if (typeof msg.count === "number") store.unreadCount = msg.count;
  });
});

onBeforeUnmount(() => {
  offAnnouncement?.();
  offCount?.();
  disconnect();
  if (pulseTimer) clearTimeout(pulseTimer);
});

watch(
  () => store.popoverQueue.length,
  (newLen, oldLen) => {
    if (newLen > oldLen) {
      pulse.value = true;
      if (pulseTimer) clearTimeout(pulseTimer);
      pulseTimer = setTimeout(() => {
        pulse.value = false;
      }, 1500);
    }
    lastQueueLen.value = newLen;
  }
);

const recentItems = computed<NotificationItem[]>(() => {
  const list = store.inboxList.length > 0 ? store.inboxList : store.popoverQueue;
  return list.slice(0, POPOVER_LIMIT);
});

function fmtRelative(ts: number): string {
  const diffMs = Date.now() - ts;
  const min = Math.floor(diffMs / 60000);
  if (min < 1) return t("menus.notificationJustNow");
  if (min < 60) return t("menus.notificationMinutesAgo", { n: min });
  const hr = Math.floor(min / 60);
  if (hr < 24) return t("menus.notificationHoursAgo", { n: hr });
  const day = Math.floor(hr / 24);
  return t("menus.notificationDaysAgo", { n: day });
}

async function markAllRead() {
  await store.readAll();
}

async function markOneRead(item: NotificationItem) {
  await store.readOne(item.id);
}

async function openItem(item: NotificationItem) {
  // mark read + navigate to detail page
  await store.readOne(item.id);
  router.push(`/notification/detail/${item.id}`);
}

function goInbox() {
  router.push("/notification/inbox");
}
</script>

<template>
  <el-popover trigger="click" :width="380" placement="bottom-end">
    <template #reference>
      <el-badge
        :value="store.unreadCount"
        :hidden="store.unreadCount === 0"
        :max="99"
      >
        <el-button
          text
          :icon="Bell"
          :class="['bell-trigger', { 'bell-pulse': pulse }]"
        />
      </el-badge>
    </template>
    <div class="notification-popover">
      <div class="flex items-center justify-between px-1 mb-2">
        <span class="text-sm font-semibold">
          {{ t("menus.notificationBellTitle") }}
        </span>
        <el-button
          v-if="store.unreadCount > 0"
          link
          type="primary"
          size="small"
          @click="markAllRead"
        >
          {{ t("menus.notificationMarkAllRead") }}
        </el-button>
      </div>

      <div
        v-if="recentItems.length === 0"
        class="text-center text-gray-400 py-6 text-sm"
      >
        {{ t("menus.notificationEmpty") }}
      </div>

      <ul v-else class="notification-list">
        <li
          v-for="item in recentItems"
          :key="item.id"
          class="notification-item"
        >
          <div
            class="notification-title"
            role="button"
            tabindex="0"
            @click="openItem(item)"
            @keydown.enter="openItem(item)"
          >
            {{ item.title }}
          </div>
          <div class="flex items-center justify-between mt-1">
            <span class="notification-time">
              {{ fmtRelative(item.received_at) }}
            </span>
            <el-button
              link
              type="primary"
              size="small"
              @click.stop="markOneRead(item)"
            >
              {{ t("menus.notificationMarkRead") }}
            </el-button>
          </div>
        </li>
      </ul>

      <div class="text-center pt-2 border-t border-gray-100 mt-2">
        <el-button link type="primary" size="small" @click="goInbox">
          {{ t("menus.notificationViewAll") }}
        </el-button>
      </div>
    </div>
  </el-popover>
</template>

<style lang="scss" scoped>
.bell-trigger {
  font-size: 18px;
}

.bell-pulse :deep(svg) {
  animation: bell-ring 1.5s ease-in-out;
  color: var(--el-color-primary);
}

@keyframes bell-ring {
  0%,
  100% {
    transform: rotate(0);
  }
  10%,
  30%,
  50%,
  70%,
  90% {
    transform: rotate(8deg);
  }
  20%,
  40%,
  60%,
  80% {
    transform: rotate(-8deg);
  }
}

.notification-popover {
  max-height: 460px;
}

.notification-list {
  max-height: 320px;
  overflow-y: auto;
  list-style: none;
  margin: 0;
  padding: 0;
}

.notification-item {
  padding: 8px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  &:last-child {
    border-bottom: none;
  }
  &:hover {
    background: var(--el-fill-color-light);
  }
}

.notification-title {
  font-size: 13px;
  color: var(--el-text-color-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  cursor: pointer;
  &:hover {
    color: var(--el-color-primary);
  }
}

.notification-time {
  font-size: 11px;
  color: var(--el-text-color-secondary);
}
</style>