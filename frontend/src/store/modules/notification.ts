import { defineStore } from "pinia";
import { ref } from "vue";
import { markRead, unreadCount as fetchUnread } from "@/api/notification";

export interface NotificationItem {
  id: number;
  title: string;
  received_at: number;
}

export const useNotificationStore = defineStore("notification", () => {
  const unreadCount = ref(0);
  const popoverQueue = ref<NotificationItem[]>([]);
  const lastFetchedAt = ref(0);

  async function refreshUnread() {
    const { count } = await fetchUnread();
    unreadCount.value = count;
    lastFetchedAt.value = Date.now();
  }

  function pushFromSocket(item: Omit<NotificationItem, "received_at">) {
    popoverQueue.value.push({ ...item, received_at: Date.now() });
    unreadCount.value += 1;
  }

  async function readOne(id: number) {
    const { changed } = await markRead(id);
    if (changed) {
      unreadCount.value = Math.max(0, unreadCount.value - 1);
    }
  }

  function dismiss(id: number) {
    popoverQueue.value = popoverQueue.value.filter(n => n.id !== id);
  }

  return {
    unreadCount,
    popoverQueue,
    lastFetchedAt,
    refreshUnread,
    pushFromSocket,
    readOne,
    dismiss
  };
});
