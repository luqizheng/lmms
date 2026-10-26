<script setup lang="ts">
export type AlertVariant = "info" | "success" | "warning" | "danger";

interface Props {
  /** 语义变体 */
  variant?: AlertVariant;
  /** 标题 */
  title?: string;
}

withDefaults(defineProps<Props>(), { variant: "info", title: undefined });
</script>

<template>
  <div class="ui-alert" :class="`ui-alert--${variant}`" role="note">
    <span class="ui-alert__bar" aria-hidden="true" />
    <div class="ui-alert__body">
      <p v-if="title" class="ui-alert__title">{{ title }}</p>
      <div class="ui-alert__content">
        <slot />
      </div>
    </div>
  </div>
</template>

<style scoped>
.ui-alert {
  display: flex;
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--surface-raised);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
}

.ui-alert__bar {
  width: 3px;
  flex: none;
  border-radius: var(--radius-full);
  background: var(--info);
}

.ui-alert--success .ui-alert__bar {
  background: var(--success);
}

.ui-alert--warning .ui-alert__bar {
  background: var(--warning);
}

.ui-alert--danger .ui-alert__bar {
  background: var(--danger);
}

.ui-alert__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.ui-alert__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-body);
  color: var(--text-primary);
}

.ui-alert__content {
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  color: var(--text-secondary);
}
</style>
