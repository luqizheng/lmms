<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

interface Props {
  /** 可排序表头：外部负责 aria-sort 与点击处理 */
  sortable?: boolean;
}

const props = withDefaults(defineProps<Props>(), { sortable: false });

const classes = computed(() =>
  cn("ui-table__th", { "is-sortable": props.sortable })
);
</script>

<template>
  <th scope="col" :class="classes">
    <slot />
  </th>
</template>

<style scoped>
/* Phase 1R §6.7：表头高 40px，12px/450 secondary，字距 +2% */
.ui-table__th {
  height: 40px;
  padding: 0 var(--space-4);
  text-align: left;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  white-space: nowrap;
}

.ui-table__th.is-sortable {
  cursor: pointer;
  user-select: none;
}
</style>
