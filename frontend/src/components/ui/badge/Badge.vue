<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

export type BadgeVariant =
  | "neutral"
  | "success"
  | "warning"
  | "danger"
  | "info"
  | "accent";

interface Props {
  /** 语义变体 — Phase 1R §6.6：subtle 底上的文字一律用 strong 档 */
  variant?: BadgeVariant;
}

const props = withDefaults(defineProps<Props>(), { variant: "neutral" });

const classes = computed(() => cn("ui-badge", `ui-badge--${props.variant}`));
</script>

<template>
  <span :class="classes">
    <slot />
  </span>
</template>

<style scoped>
/* Phase 1R §6.6 */
.ui-badge {
  display: inline-flex;
  align-items: center;
  height: var(--size-badge);
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  letter-spacing: var(--tracking-caption);
  white-space: nowrap;
}

.ui-badge--neutral {
  background: var(--surface-sunken);
  color: var(--text-secondary);
}

.ui-badge--success {
  background: var(--success-subtle);
  color: var(--success-strong);
}

.ui-badge--warning {
  background: var(--warning-subtle);
  color: var(--warning-strong);
}

.ui-badge--danger {
  background: var(--danger-subtle);
  color: var(--danger-strong);
}

.ui-badge--info {
  background: var(--info-subtle);
  color: var(--info-strong);
}

.ui-badge--accent {
  background: var(--accent-subtle);
  color: var(--accent-deep);
  border: 1px solid var(--border-accent-glow);
}
</style>
