<script setup lang="ts">
import {
  TooltipContent,
  TooltipPortal,
  TooltipProvider,
  TooltipRoot,
  TooltipTrigger
} from "reka-ui";

interface Props {
  /** 浮层文案（由调用方注入 i18n，组件自身不硬编码中文） */
  content?: string;
  /** 受控展开状态 */
  open?: boolean;
  /** 禁用提示 */
  disabled?: boolean;
  /** 悬停延迟（毫秒） */
  delayDuration?: number;
  /** 浮层相对触发器的位置 */
  side?: "top" | "right" | "bottom" | "left";
  /** 浮层对齐方式 */
  align?: "start" | "center" | "end";
  /** 触发器的无障碍名称（图标按钮场景） */
  ariaLabel?: string;
  /**
   * 触发器是否只含图标（无可见文字）。
   * 开启时把浮层文案渲染为读屏专用文本，作为按钮的可访问名称。
   */
  iconOnly?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  content: undefined,
  open: undefined,
  disabled: false,
  delayDuration: 300,
  side: "top",
  align: "center",
  ariaLabel: undefined,
  iconOnly: false
});

const emit = defineEmits<{ "update:open": [value: boolean] }>();
</script>

<template>
  <TooltipProvider :delay-duration="delayDuration">
    <TooltipRoot
      :open="open"
      :disabled="disabled"
      @update:open="emit('update:open', $event)"
    >
      <!-- 触发器：原生 button，键盘可达并自带焦点环 -->
      <TooltipTrigger
        class="ui-tooltip__trigger"
        :aria-label="ariaLabel"
        :data-disabled="disabled ? '' : undefined"
      >
        <slot />
        <!-- 读屏专用文本：图标按钮场景下由浮层文案兜底命名（已有 aria-label 时不再重复） -->
        <span v-if="iconOnly && content && !ariaLabel" class="ui-sr-only">{{
          content
        }}</span>
      </TooltipTrigger>

      <TooltipPortal>
        <!-- 浮层 — Phase 1R §6.4：白底 + border-strong + shadow-md / radius-md / 内边距 6px -->
        <TooltipContent
          class="ui-tooltip__content"
          :side="side"
          :align="align"
          :side-offset="8"
        >
          <slot name="content">{{ content }}</slot>
        </TooltipContent>
      </TooltipPortal>
    </TooltipRoot>
  </TooltipProvider>
</template>

<style scoped>
/* 触发器 — 轻量的 ghost 形态：可包裹图标或文字，焦点环始终可见 */
.ui-tooltip__trigger {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-1);
  padding: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
  background: transparent;
  border: var(--border-1) solid transparent;
  border-radius: var(--radius-xs);
  cursor: help;
}

.ui-tooltip__trigger:hover:not([data-disabled]) {
  color: var(--text-secondary);
}

/* 焦点环 — Phase 1R §5.5 / Phase 2 §5.2 */
.ui-tooltip__trigger:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
  box-shadow: 0 0 0 var(--size-focus-halo) var(--focus-glow);
}

.ui-tooltip__trigger[data-disabled] {
  color: var(--text-disabled);
  cursor: not-allowed;
}

/* 浮层容器 — Phase 1R §6.4 / §5.1 E2（reka-ui 浮层被 Portal 传送，需用 :deep 命中） */
:deep(.ui-tooltip__content) {
  max-width: calc(var(--space-12) * 5);
  padding: var(--size-menu-padding);
  background: var(--surface-overlay);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-md);
  box-shadow:
    inset 0 1px 0 0 var(--surface-highlight),
    var(--shadow-md);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-caption);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  animation: ui-tooltip-enter var(--duration-fast) var(--ease-decelerate);
}

@keyframes ui-tooltip-enter {
  from {
    opacity: 0;
  }

  to {
    opacity: 1;
  }
}

/* 读屏专用文本 —— 不依赖尚未引入的全局 base.css */
.ui-sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
  border: 0;
}

/* 减少动效 — Phase 2 §5.4 #4：仅 opacity 80ms */
@media (prefers-reduced-motion: reduce) {
  :deep(.ui-tooltip__content) {
    animation: ui-tooltip-enter-instant var(--duration-instant) linear;
  }

  @keyframes ui-tooltip-enter-instant {
    from {
      opacity: 0;
    }

    to {
      opacity: 1;
    }
  }
}
</style>
