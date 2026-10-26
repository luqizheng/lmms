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

/**
 * 页码窗口（总数大时不能把每一页都渲染出来）。
 *
 * 形如 1 … 4 5 [6] 7 8 … 20：首尾各留一格，当前页左右各两格。
 */
const pages = computed<(number | "gap")[]>(() => {
  const total = totalPages.value;
  const current = props.page;
  const window = 2;

  if (total <= 7) {
    return Array.from({ length: total }, (_, index) => index + 1);
  }

  const result: (number | "gap")[] = [];
  const keep = new Set<number>([1, total]);
  for (let i = current - window; i <= current + window; i++) {
    if (i >= 1 && i <= total) keep.add(i);
  }

  let previous = 0;
  for (const page of [...keep].sort((a, b) => a - b)) {
    if (previous && page - previous > 1) result.push("gap");
    result.push(page);
    previous = page;
  }
  return result;
});

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
    <template v-for="(p, index) in pages" :key="`${p}-${index}`">
      <span v-if="p === 'gap'" class="ui-pagination__gap" aria-hidden="true"
        >…</span
      >
      <button
        v-else
        type="button"
        class="ui-pagination__button ui-pagination__button--page"
        :class="{ 'is-current': p === props.page }"
        :aria-current="p === props.page ? 'page' : undefined"
        :aria-label="`第 ${p} 页`"
        @click="go(p)"
      >
        {{ p }}
      </button>
    </template>
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

.ui-pagination__gap {
  padding: 0 var(--space-1);
  color: var(--text-tertiary);
}

.ui-pagination__button.is-current {
  background: var(--accent-subtle);
  border-color: var(--border-accent-glow);
  color: var(--accent-deep);
}
</style>
