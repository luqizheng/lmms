<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";
import {
  BUTTON_SIZES,
  BUTTON_VARIANTS,
  type ButtonSize,
  type ButtonVariant
} from "./button";

interface Props {
  /** 视觉变体 — Phase 1R §6.1 */
  variant?: ButtonVariant;
  /** 尺寸系 — Phase 1R §6.1：32 / 36 / 40 / 图标方钮 */
  size?: ButtonSize;
  /** 异步进行中：禁用按钮并标记 aria-busy */
  loading?: boolean;
  disabled?: boolean;
  type?: "button" | "submit" | "reset";
}

const props = withDefaults(defineProps<Props>(), {
  variant: "primary",
  size: "md",
  loading: false,
  disabled: false,
  type: "button"
});

const isDisabled = computed(() => props.disabled || props.loading);
</script>

<template>
  <button
    :type="type"
    :disabled="isDisabled"
    :aria-busy="loading ? 'true' : undefined"
    :class="
      cn('ui-button', `ui-button--${variant}`, `ui-button--${size}`, {
        'is-loading': loading
      })
    "
  >
    <span v-if="loading" class="ui-button__spinner" aria-hidden="true" />
    <slot />
  </button>
</template>

<style scoped>
.ui-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  border: 1px solid transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-body);
  border-radius: var(--radius-sm);
  padding: 0 var(--space-4);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    border-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate),
    box-shadow var(--duration-fast) var(--ease-decelerate),
    transform var(--duration-instant) var(--ease-standard);
}

.ui-button:disabled {
  cursor: not-allowed;
}

/* Primary — Phase 1R §6.1 */
.ui-button--primary {
  background: var(--accent-gradient);
  color: var(--accent-contrast-text);
  box-shadow: var(--shadow-accent);
}

.ui-button--primary:hover:not(:disabled) {
  background: var(--accent-gradient-hover);
  box-shadow: var(--shadow-accent-lg);
}

.ui-button--primary:active:not(:disabled) {
  background: var(--accent-gradient-pressed);
  box-shadow: var(--shadow-accent);
  transform: scale(0.98);
}

.ui-button--primary:disabled {
  background: var(--surface-disabled);
  color: var(--text-disabled);
  box-shadow: none;
}

/* Secondary — Phase 1R §6.1 */
.ui-button--secondary {
  background: var(--surface-raised);
  border-color: var(--border-strong);
  color: var(--text-primary);
}

.ui-button--secondary:hover:not(:disabled) {
  background: var(--surface-hover-veil);
  border-color: var(--border-hover-strong);
}

.ui-button--secondary:active:not(:disabled) {
  background: var(--surface-sunken);
  transform: scale(0.98);
}

.ui-button--secondary:disabled {
  background: var(--surface-disabled);
  color: var(--text-disabled);
  border-color: transparent;
}

/* Ghost — Phase 1R §6.1 */
.ui-button--ghost {
  background: transparent;
  color: var(--text-secondary);
}

.ui-button--ghost:hover:not(:disabled) {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-button--ghost:active:not(:disabled) {
  background: var(--surface-active-veil);
}

.ui-button--ghost:disabled {
  color: var(--text-disabled);
}

/* 尺寸系 — Phase 1R §6.1 */
.ui-button--sm {
  height: 32px;
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  padding: 0 var(--space-3);
}

.ui-button--md {
  height: 36px;
}

.ui-button--lg {
  height: 40px;
  font-size: var(--font-size-body-strong);
  line-height: var(--line-height-body-strong);
}

.ui-button--icon {
  width: 36px;
  height: 36px;
  padding: 0;
}

/* 加载指示 — Phase 1R §6.1：14px spinner，800ms 一圈 */
.ui-button__spinner {
  flex: none;
  width: 14px;
  height: 14px;
  border: 2px solid currentcolor;
  border-right-color: transparent;
  border-radius: var(--radius-full);
  animation: ui-button-spin 800ms linear infinite;
}

@keyframes ui-button-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
