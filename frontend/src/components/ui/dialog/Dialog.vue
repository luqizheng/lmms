<script setup lang="ts">
import {
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle
} from "reka-ui";

interface Props {
  /** 是否打开（v-model:open） */
  open?: boolean;
  /** 弹框标题 */
  title?: string;
  /** 副标题 / 说明 */
  description?: string;
  /** 关闭按钮的无障碍名称 */
  closeLabel?: string;
  /** 宽度档位 */
  size?: "sm" | "md" | "lg";
}

const props = withDefaults(defineProps<Props>(), {
  open: false,
  title: undefined,
  description: undefined,
  closeLabel: "关闭",
  size: "md"
});

const emit = defineEmits<{ "update:open": [value: boolean] }>();
</script>

<template>
  <DialogRoot :open="props.open" @update:open="emit('update:open', $event)">
    <DialogPortal>
      <DialogOverlay class="ui-dialog__overlay" />
      <DialogContent
        class="ui-dialog__content"
        :class="`ui-dialog--${props.size}`"
      >
        <header class="ui-dialog__header">
          <DialogTitle class="ui-dialog__title">{{ props.title }}</DialogTitle>
          <DialogClose class="ui-dialog__close" :aria-label="props.closeLabel">
            <slot name="close">×</slot>
          </DialogClose>
        </header>
        <DialogDescription
          v-if="props.description"
          class="ui-dialog__description"
        >
          {{ props.description }}
        </DialogDescription>
        <div class="ui-dialog__body">
          <slot />
        </div>
        <footer v-if="$slots.footer" class="ui-dialog__footer">
          <slot name="footer" />
        </footer>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<style scoped>
/* 遮罩 — Phase 2 §4.2：暖黑纱压灰不压黑 */
.ui-dialog__overlay {
  position: fixed;
  inset: 0;
  z-index: 50;
  background: var(--surface-ink);
  opacity: 0.28;
  animation: ui-dialog-overlay-in var(--duration-medium) var(--ease-decelerate);
}

/* 浮层 E3 — Phase 1R §5.2 */
.ui-dialog__content {
  position: fixed;
  top: 50%;
  left: 50%;
  z-index: 51;
  display: flex;
  flex-direction: column;
  width: calc(100% - var(--space-8));
  max-height: calc(100vh - var(--space-16));
  transform: translate(-50%, -50%);
  background: var(--surface-raised);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  animation: ui-dialog-in var(--duration-slow) var(--ease-decelerate);
}

.ui-dialog--sm {
  max-width: 420px;
}

.ui-dialog--md {
  max-width: 640px;
}

.ui-dialog--lg {
  max-width: 960px;
}

.ui-dialog__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  padding: var(--space-5) var(--space-6);
  border-bottom: 1px solid var(--border-subtle);
}

.ui-dialog__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h4);
  line-height: var(--line-height-h4);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h4);
  color: var(--text-primary);
}

.ui-dialog__description {
  margin: 0;
  padding: var(--space-3) var(--space-6) 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-small);
  color: var(--text-secondary);
}

.ui-dialog__body {
  flex: 1;
  padding: var(--space-6);
  overflow: auto;
}

.ui-dialog__footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-6);
  border-top: 1px solid var(--border-subtle);
}

.ui-dialog__close {
  flex: none;
  width: var(--space-6);
  height: var(--space-6);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--font-size-body);
  line-height: 1;
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

.ui-dialog__close:hover {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-dialog__close:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}

@keyframes ui-dialog-overlay-in {
  from {
    opacity: 0;
  }
  to {
    opacity: 0.28;
  }
}

@keyframes ui-dialog-in {
  from {
    opacity: 0;
    transform: translate(-50%, calc(-50% + var(--space-2)));
  }
  to {
    opacity: 1;
    transform: translate(-50%, -50%);
  }
}

/* 减少动效 — Phase 2 §5.4：仅保留 opacity */
@media (prefers-reduced-motion: reduce) {
  .ui-dialog__overlay,
  .ui-dialog__content {
    animation: none;
  }
}
</style>
