import { ref, watch } from "vue";

const STORAGE_KEY = "notification_inbox_read_ids";

function load(): Set<number> {
  if (typeof localStorage === "undefined") return new Set();
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return new Set();
    const arr = JSON.parse(raw);
    return new Set(Array.isArray(arr) ? arr : []);
  } catch {
    return new Set();
  }
}

function save(set: Set<number>) {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify([...set]));
  } catch {
    // quota or private mode — skip
  }
}

const readIds = ref<Set<number>>(load());

watch(
  readIds,
  set => {
    save(set);
  },
  { deep: true }
);

export function useInboxReadState() {
  return {
    isRead(id: number): boolean {
      return readIds.value.has(id);
    },
    markRead(id: number) {
      if (readIds.value.has(id)) return;
      const next = new Set(readIds.value);
      next.add(id);
      readIds.value = next;
    },
    unreadIds(allIds: number[]): number[] {
      return allIds.filter(id => !readIds.value.has(id));
    }
  };
}
