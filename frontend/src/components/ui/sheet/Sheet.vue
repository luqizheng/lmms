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
  /** 抽屉标题 */
  title?: string;
  /** 说明 */
  description?: string;
  /** 关闭按钮的无障碍名称 */
  closeLabel?: string;
  /** 停靠侧 */
  side?: "left" | "right";
}

const props = withDefaults(defineProps<Props>(), {
  open: false,
  title: undefined,
  description: undefined,
  closeLabel: "关闭",
  side: "left"
});

const emit = defineEmits<{ "update:open": [value: boolean] }>();
</script>

<template>
  <DialogRoot :open="props.open" @update:open="emit('update:open', $event)">
    <DialogPortal>
      <DialogOverlay class="ui-sheet__overlay" />
      <DialogContent
        class="ui-sheet"
        :class="`ui-sheet--${props.side}`"
        :data-side="props.side"
      >
        <header class="ui-sheet__header">
          <DialogTitle class="ui-sheet__title">{{ props.title }}</DialogTitle>
          <DialogClose class="ui-sheet__close" :aria-label="props.closeLabel">
            <slot name="close">×</slot>
          </DialogClose>
        </header>
        <DialogDescription
          v-if="props.description"
          class="ui-sheet__description"
        >
          {{ props.description }}
        </DialogDescription>
        <div class="ui-sheet__body">
          <slot />
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<style scoped>
/* 遮罩 — Phase 2 §4.2：暖黑纱 */
.ui-sheet__overlay {
  position: fixed;
  inset: 0;
  z-index: 50;
  background: var(--surface-ink);
  opacity: 0.28;
  animation: ui-sheet-overlay-in var(--duration-medium) var(--ease-decelerate);
}

/* 抽屉：白底 + border-default + shadow-md（Phase 2 §4.2 侧栏抽屉） */
.ui-sheet {
  position: fixed;
  top: 0;
  bottom: 0;
  z-index: 51;
  display: flex;
  flex-direction: column;
  width: var(--size-sidebar);
  background: var(--surface-raised);
  border-right: 1px solid var(--border-default);
  box-shadow: var(--shadow-md);
}

.ui-sheet--left {
  left: 0;
  animation: ui-sheet-in-left var(--duration-medium) var(--ease-decelerate);
}

.ui-sheet--right {
  right: 0;
  border-right: 0;
  border-left: 1px solid var(--border-default);
  animation: ui-sheet-in-right var(--duration-medium) var(--ease-decelerate);
}

.ui-sheet__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  padding: var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
}

.ui-sheet__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h4);
  line-height: var(--line-height-h4);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h4);
  color: var(--text-primary);
}

.ui-sheet__description {
  margin: 0;
  padding: var(--space-3) var(--space-4) 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  color: var(--text-secondary);
}

.ui-sheet__body {
  flex: 1;
  padding: var(--space-4);
  overflow: auto;
}

.ui-sheet__close {
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
}

.ui-sheet__close:hover {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-sheet__close:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}

@keyframes ui-sheet-overlay-in {
  from {
    opacity: 0;
  }
  to {
    opacity: 0.28;
  }
}

@keyframes ui-sheet-in-left {
  from {
    transform: translateX(calc(var(--size-sidebar) * -1));
  }
  to {
    transform: translateX(0);
  }
}

@keyframes ui-sheet-in-right {
  from {
    transform: translateX(var(--size-sidebar));
  }
  to {
    transform: translateX(0);
  }
}

/* 减少动效 — Phase 2 §5.4 */
@media (prefers-reduced-motion: reduce) {
  .ui-sheet__overlay,
  .ui-sheet {
    animation: none;
  }
}
</style>
