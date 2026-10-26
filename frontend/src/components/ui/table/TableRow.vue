<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

interface Props {
  /** 选中行：accent-subtle 底 + 最左 2px accent 竖条 */
  selected?: boolean;
}

const props = withDefaults(defineProps<Props>(), { selected: false });

const classes = computed(() =>
  cn("ui-table__row", { "is-selected": props.selected })
);
</script>

<template>
  <tr :class="classes">
    <slot />
  </tr>
</template>

<style scoped>
/* Phase 1R §6.7：行高 44px，行分隔 border-subtle，hover 加灰 */
.ui-table__row {
  height: 44px;
  border-bottom: 1px solid var(--border-subtle);
  transition: background-color var(--duration-fast) var(--ease-decelerate);
}

.ui-table__row:hover {
  background-color: var(--surface-hover-veil);
}

.ui-table__row.is-selected {
  background-color: var(--accent-subtle);
  box-shadow: inset 2px 0 0 0 var(--accent);
}
</style>
