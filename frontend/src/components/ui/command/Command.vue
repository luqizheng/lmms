<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";

export interface CommandItem {
  id: string;
  label: string;
  group?: string;
  shortcut?: string;
}

interface Props {
  /** 是否打开（v-model:open） */
  open?: boolean;
  /** 命令条目 */
  items: CommandItem[];
  /** 搜索框占位文案 */
  placeholder?: string;
  /** 无结果提示 */
  emptyText?: string;
  /** 无障碍名称 */
  label?: string;
}

const props = withDefaults(defineProps<Props>(), {
  open: false,
  placeholder: "搜索…",
  emptyText: "未找到匹配结果",
  label: "命令面板"
});

const emit = defineEmits<{
  "update:open": [value: boolean];
  select: [id: string];
}>();

const query = ref("");
const activeIndex = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return props.items;
  return props.items.filter(
    item =>
      item.label.toLowerCase().includes(q) ||
      (item.group ?? "").toLowerCase().includes(q)
  );
});

watch(filtered, () => {
  activeIndex.value = 0;
});

watch(
  () => props.open,
  async open => {
    if (!open) return;
    query.value = "";
    activeIndex.value = 0;
    await nextTick();
    inputRef.value?.focus();
  },
  { immediate: true }
);

function onInput(event: Event) {
  query.value = (event.target as HTMLInputElement).value;
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "ArrowDown") {
    event.preventDefault();
    activeIndex.value = Math.min(
      activeIndex.value + 1,
      filtered.value.length - 1
    );
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    activeIndex.value = Math.max(activeIndex.value - 1, 0);
  } else if (event.key === "Enter") {
    event.preventDefault();
    const item = filtered.value[activeIndex.value];
    if (item) {
      emit("select", item.id);
      emit("update:open", false);
    }
  } else if (event.key === "Escape") {
    event.preventDefault();
    emit("update:open", false);
  }
}
</script>

<template>
  <div
    v-if="props.open"
    class="ui-command"
    role="dialog"
    :aria-label="props.label"
  >
    <div class="ui-command__panel">
      <input
        ref="inputRef"
        class="ui-command__input"
        type="text"
        role="combobox"
        aria-expanded="true"
        :aria-label="props.label"
        :placeholder="props.placeholder"
        :value="query"
        @input="onInput"
        @keydown="onKeydown"
      />
      <ul class="ui-command__list" role="listbox">
        <li
          v-for="(item, index) in filtered"
          :key="item.id"
          class="ui-command__item"
          :class="{ 'is-active': index === activeIndex }"
          role="option"
          :aria-selected="index === activeIndex ? 'true' : 'false'"
          @click="
            emit('select', item.id);
            emit('update:open', false);
          "
        >
          <span class="ui-command__item-label">{{ item.label }}</span>
          <span v-if="item.group" class="ui-command__item-group">{{
            item.group
          }}</span>
          <kbd v-if="item.shortcut" class="ui-command__item-shortcut">{{
            item.shortcut
          }}</kbd>
        </li>
      </ul>
      <p v-if="filtered.length === 0" class="ui-command__empty">
        {{ props.emptyText }}
      </p>
    </div>
  </div>
</template>

<style scoped>
/* 命令面板 — Phase 2 §3.2 / §5.2：E3 规格（白底 + shadow-lg） */
.ui-command {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 15vh;
  background: var(--surface-ink);
  opacity: 0.28;
}

.ui-command__panel {
  width: calc(100% - var(--space-8));
  max-width: 640px;
  max-height: 60vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--surface-raised);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
}

.ui-command__input {
  width: 100%;
  height: var(--size-control-lg);
  padding: 0 var(--space-4);
  border: 0;
  border-bottom: 1px solid var(--border-subtle);
  background: var(--surface-raised);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.ui-command__input::placeholder {
  color: var(--text-tertiary);
}

.ui-command__input:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: calc(var(--size-focus-ring) * -1);
}

.ui-command__list {
  margin: 0;
  padding: var(--space-2);
  list-style: none;
  overflow-y: auto;
}

.ui-command__item {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  height: 32px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-xs);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-secondary);
  cursor: pointer;
}

.ui-command__item.is-active {
  background: var(--accent-subtle);
  color: var(--accent-deep);
}

.ui-command__item-label {
  flex: 1;
}

.ui-command__item-group {
  font-size: var(--font-size-caption);
  color: var(--text-tertiary);
}

.ui-command__item-shortcut {
  padding: 0 var(--space-1);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-xs);
  background: var(--surface-base);
  box-shadow: inset 0 -1px 0 var(--border-strong);
  font-family: var(--font-mono);
  font-size: var(--font-size-kbd);
  color: var(--text-secondary);
}

.ui-command__empty {
  margin: 0;
  padding: var(--space-4);
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  color: var(--text-secondary);
}
</style>
