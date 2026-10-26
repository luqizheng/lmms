<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

interface Props {
  /** 可交互卡片：hover 升描边与阴影，不做位移 */
  interactive?: boolean;
}

const props = withDefaults(defineProps<Props>(), { interactive: false });

const classes = computed(() =>
  cn("ui-card", { "is-interactive": props.interactive })
);
</script>

<template>
  <section :class="classes">
    <header v-if="$slots.header" class="ui-card__header">
      <slot name="header" />
    </header>
    <div class="ui-card__body">
      <slot />
    </div>
    <footer v-if="$slots.footer" class="ui-card__footer">
      <slot name="footer" />
    </footer>
  </section>
</template>

<style scoped>
/* Phase 1R §6.3 */
.ui-card {
  background: var(--surface-raised);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-xs);
  transition:
    border-color var(--duration-fast) var(--ease-decelerate),
    box-shadow var(--duration-fast) var(--ease-decelerate);
}

.ui-card__header {
  padding: var(--space-6) var(--space-6) var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
  font-family: var(--font-ui);
  font-size: var(--font-size-h4);
  line-height: var(--line-height-h4);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h4);
  color: var(--text-primary);
}

.ui-card__body {
  padding: var(--space-6);
}

.ui-card__footer {
  padding: var(--space-4) var(--space-6) var(--space-6);
  border-top: 1px solid var(--border-subtle);
}

.ui-card.is-interactive:hover {
  border-color: var(--border-strong);
  box-shadow: var(--shadow-sm);
}
</style>
