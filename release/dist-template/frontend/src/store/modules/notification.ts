import { defineStore } from "pinia";
import { ref, onMounted, onUnmounted } from "vue";
import {
  markRead,
  unreadCount as fetchUnread,
  listAnnouncements
} from "@/api/notification";

export interface NotificationItem {
  id: number;
  title: string;
  received_at: number;
}

interface SyncEvent {
  type: "read" | "clear" | "refresh";
  id?: number;
  timestamp: number;
}

const STORAGE_KEY = "notification_sync";

function broadcastEvent(event: SyncEvent) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(event));
    localStorage.removeItem(STORAGE_KEY);
  } catch {}
}

export const useNotificationStore = defineStore("notification", () => {
  const unreadCount = ref(0);
  const popoverQueue = ref<NotificationItem[]>([]);
  const lastFetchedAt = ref(0);
  const inboxList = ref<NotificationItem[]>([]);

  async function refreshUnread() {
    const { count } = await fetchUnread();
    unreadCount.value = count;
    lastFetchedAt.value = Date.now();
  }

  async function refreshInbox() {
    const items = await listAnnouncements();
    inboxList.value = items.map(item => ({
      id: item.id,
      title: item.title,
      received_at: new Date(item.publish_at).getTime()
    }));
  }

  function pushFromSocket(item: Omit<NotificationItem, "received_at">) {
    const exists = popoverQueue.value.some(n => n.id === item.id);
    if (!exists) {
      popoverQueue.value.push({ ...item, received_at: Date.now() });
      unreadCount.value += 1;
    }
  }

  async function readOne(id: number) {
    const { changed } = await markRead(id);
    if (changed) {
      unreadCount.value = Math.max(0, unreadCount.value - 1);
      popoverQueue.value = popoverQueue.value.filter(n => n.id !== id);
      broadcastEvent({ type: "read", id, timestamp: Date.now() });
    }
  }

  async function readAll() {
    const promises = popoverQueue.value.map(item => markRead(item.id));
    await Promise.all(promises);
    unreadCount.value = 0;
    popoverQueue.value = [];
    broadcastEvent({ type: "clear", timestamp: Date.now() });
  }

  function dismiss(id: number) {
    popoverQueue.value = popoverQueue.value.filter(n => n.id !== id);
  }

  function clearAll() {
    popoverQueue.value = [];
    broadcastEvent({ type: "clear", timestamp: Date.now() });
  }

  function handleSyncEvent(e: StorageEvent) {
    if (e.key !== STORAGE_KEY) return;
    try {
      const event: SyncEvent = JSON.parse(e.newValue || "{}");
      if (event.type === "read" && event.id) {
        popoverQueue.value = popoverQueue.value.filter(n => n.id !== event.id);
        if (unreadCount.value > 0) unreadCount.value -= 1;
      } else if (event.type === "clear") {
        popoverQueue.value = [];
        unreadCount.value = 0;
      } else if (event.type === "refresh") {
        refreshUnread().catch(() => undefined);
      }
    } catch {}
  }

  onMounted(() => {
    window.addEventListener("storage", handleSyncEvent);
  });

  onUnmounted(() => {
    window.removeEventListener("storage", handleSyncEvent);
  });

  return {
    unreadCount,
    popoverQueue,
    inboxList,
    lastFetchedAt,
    refreshUnread,
    refreshInbox,
    pushFromSocket,
    readOne,
    readAll,
    dismiss,
    clearAll
  };
});
