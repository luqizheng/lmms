<script setup lang="ts">
import { computed } from "vue";

interface Props {
  /** 当前页码（1 起） */
  page: number;
  /** 总条数 */
  total: number;
  /** 每页条数 */
  pageSize: number;
  /** 导航的无障碍名称 */
  ariaLabel?: string;
  /** 上一页文案 */
  prevLabel?: string;
  /** 下一页文案 */
  nextLabel?: string;
}

const props = withDefaults(defineProps<Props>(), {
  ariaLabel: "分页",
  prevLabel: "上一页",
  nextLabel: "下一页"
});

const emit = defineEmits<{ "update:page": [value: number] }>();

const totalPages = computed(() =>
  Math.max(1, Math.ceil(props.total / props.pageSize))
);

const pages = computed(() =>
  Array.from({ length: totalPages.value }, (_, index) => index + 1)
);

function go(page: number) {
  if (page < 1 || page > totalPages.value || page === props.page) return;
  emit("update:page", page);
}
</script>

<template>
  <nav class="ui-pagination" :aria-label="ariaLabel">
    <button
      type="button"
      class="ui-pagination__button"
      :disabled="props.page <= 1"
      :aria-label="props.prevLabel"
      @click="go(props.page - 1)"
    >
      ‹
    </button>
    <button
      v-for="p in pages"
      :key="p"
      type="button"
      class="ui-pagination__button ui-pagination__button--page"
      :class="{ 'is-current': p === props.page }"
      :aria-current="p === props.page ? 'page' : undefined"
      :aria-label="`第 ${p} 页`"
      @click="go(p)"
    >
      {{ p }}
    </button>
    <button
      type="button"
      class="ui-pagination__button"
      :disabled="props.page >= totalPages"
      :aria-label="props.nextLabel"
      @click="go(props.page + 1)"
    >
      ›
    </button>
  </nav>
</template>

<style scoped>
/* 分页器 — Phase 2 §3.3：页码 mono，当前页 accent-deep */
.ui-pagination {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
}

.ui-pagination__button {
  min-width: var(--size-control-md);
  height: var(--size-control-md);
  padding: 0 var(--space-2);
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: transparent;
  font-family: var(--font-mono);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  font-variant-numeric: tabular-nums;
  color: var(--text-secondary);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

.ui-pagination__button:hover:not(:disabled) {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-pagination__button:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}

.ui-pagination__button:disabled {
  color: var(--text-disabled);
  cursor: not-allowed;
}

.ui-pagination__button.is-current {
  background: var(--accent-subtle);
  border-color: var(--border-accent-glow);
  color: var(--accent-deep);
}
</style>
