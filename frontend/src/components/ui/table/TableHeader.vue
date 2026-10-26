<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

interface Props {
  /** 可排序表头：内容渲染为可聚焦按钮（键盘可达，Phase 2 §9.4） */
  sortable?: boolean;
  /** 当前排序状态，输出到 aria-sort（Phase 2 §5.3） */
  sort?: "none" | "ascending" | "descending";
}

const props = withDefaults(defineProps<Props>(), {
  sortable: false,
  sort: undefined
});

const classes = computed(() =>
  cn("ui-table__th", { "is-sortable": props.sortable })
);

const ariaSort = computed(
  () => props.sort ?? (props.sortable ? ("none" as const) : undefined)
);
</script>

<template>
  <th scope="col" :aria-sort="ariaSort" :class="classes">
    <button v-if="sortable" type="button" class="ui-table__sort-button">
      <slot />
    </button>
    <slot v-else />
  </th>
</template>

<style scoped>
/* Phase 1R §6.7：表头高 40px，12px/450 secondary，字距 +2% */
.ui-table__th {
  height: var(--size-header-cell);
  padding: 0 var(--space-4);
  text-align: left;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-caption);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  white-space: nowrap;
}

.ui-table__th.is-sortable {
  cursor: pointer;
  user-select: none;
}

.ui-table__sort-button {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 0;
  border: 0;
  background: transparent;
  font: inherit;
  letter-spacing: inherit;
  color: inherit;
  cursor: pointer;
}

.ui-table__sort-button:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
  border-radius: var(--radius-xs);
}
</style>
