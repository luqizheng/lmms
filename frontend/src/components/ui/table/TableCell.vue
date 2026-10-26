<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

interface Props {
  /** 数值列：mono + tabular-nums + 右对齐（Phase 1R §3.1 / §6.7） */
  numeric?: boolean;
}

const props = withDefaults(defineProps<Props>(), { numeric: false });

const classes = computed(() =>
  cn("ui-table__td", { "is-numeric": props.numeric })
);
</script>

<template>
  <td :class="classes">
    <slot />
  </td>
</template>

<style scoped>
.ui-table__td {
  padding: 0 var(--space-4);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.ui-table__td.is-numeric {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  text-align: right;
}
</style>
