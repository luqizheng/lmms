import { ref } from "vue";
import { listAnnouncements } from "@/api/notification";
import type { Announcement } from "@/api/notification";

const STORAGE_KEY = "popover_announcement_ids";

function getDismissedIds(): Set<number> {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) {
      return new Set(JSON.parse(stored));
    }
  } catch {}
  return new Set();
}

function saveDismissedIds(ids: Set<number>) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify([...ids]));
  } catch {}
}

export function usePopoverAnnouncement() {
  const visible = ref(false);
  const currentId = ref<number | null>(null);
  const queue = ref<Announcement[]>([]);
  const dismissedIds = ref(getDismissedIds());

  async function loadPopoverAnnouncements() {
    try {
      const announcements = await listAnnouncements();
      queue.value = announcements
        .filter(a => a.popover && a.status === "published")
        .filter(a => !dismissedIds.value.has(a.id))
        .sort((a, b) => {
          if (a.pinned && !b.pinned) return -1;
          if (!a.pinned && b.pinned) return 1;
          return (
            new Date(b.publish_at).getTime() - new Date(a.publish_at).getTime()
          );
        });
      if (queue.value.length > 0) {
        showNext();
      }
    } catch {}
  }

  function showNext() {
    if (queue.value.length > 0) {
      const next = queue.value.shift()!;
      currentId.value = next.id;
      visible.value = true;
    } else {
      visible.value = false;
      currentId.value = null;
    }
  }

  function handleClose() {
    if (currentId.value) {
      dismissedIds.value.add(currentId.value);
      saveDismissedIds(dismissedIds.value);
    }
    visible.value = false;
    setTimeout(() => {
      showNext();
    }, 300);
  }

  function handleViewDetail() {
    if (currentId.value) {
      dismissedIds.value.add(currentId.value);
      saveDismissedIds(dismissedIds.value);
    }
    visible.value = false;
  }

  function reset() {
    queue.value = [];
    visible.value = false;
    currentId.value = null;
  }

  return {
    visible,
    currentId,
    handleClose,
    handleViewDetail,
    loadPopoverAnnouncements,
    reset
  };
}
