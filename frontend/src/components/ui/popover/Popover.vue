<script setup lang="ts">
import {
  PopoverArrow,
  PopoverContent,
  PopoverPortal,
  PopoverRoot,
  PopoverTrigger
} from "reka-ui";

interface Props {
  /** 浮层标题（由调用方注入 i18n，组件自身不硬编码中文） */
  title?: string;
  /** 受控展开状态 */
  open?: boolean;
  /** 模态模式：展开时锁定背景交互 */
  modal?: boolean;
  /** 浮层相对触发器的位置 */
  side?: "top" | "right" | "bottom" | "left";
  /** 浮层对齐方式 */
  align?: "start" | "center" | "end";
  /** 是否显示箭头 */
  arrow?: boolean;
  /** 触发器的无障碍名称（图标按钮场景） */
  ariaLabel?: string;
  /**
   * 触发器是否只含图标（无可见文字）。
   * 开启时把标题渲染为读屏专用文本，作为按钮的可访问名称。
   */
  iconOnly?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  title: undefined,
  open: undefined,
  modal: false,
  side: "bottom",
  align: "center",
  arrow: false,
  ariaLabel: undefined,
  iconOnly: false
});

const emit = defineEmits<{ "update:open": [value: boolean] }>();
</script>

<template>
  <PopoverRoot
    :open="open"
    :modal="modal"
    @update:open="emit('update:open', $event)"
  >
    <!-- 触发器：原生 button，键盘可达并自带焦点环 -->
    <PopoverTrigger class="ui-popover__trigger" :aria-label="ariaLabel">
      <slot />
      <!-- 读屏专用文本：图标按钮场景下由标题兜底命名 -->
      <span v-if="iconOnly && title" class="ui-sr-only">{{ title }}</span>
    </PopoverTrigger>

    <PopoverPortal>
      <!-- 浮层 — Phase 1R §6.4：白底 + border-strong + shadow-md / radius-md / 内边距 6px -->
      <PopoverContent
        class="ui-popover__content"
        :side="side"
        :align="align"
        :side-offset="8"
      >
        <div v-if="title" class="ui-popover__title">{{ title }}</div>
        <div class="ui-popover__body">
          <slot name="content" />
        </div>
        <!-- 箭头 — 底色 --surface-raised / 描边 --border-strong -->
        <PopoverArrow
          v-if="arrow"
          class="ui-popover__arrow"
          :width="12"
          :height="6"
          rounded
        />
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>

<style scoped>
.ui-popover__trigger {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  padding: 0 var(--space-3);
  height: var(--size-control-md);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    border-color var(--duration-fast) var(--ease-decelerate);
}

.ui-popover__trigger:hover {
  background: var(--surface-hover-veil);
  border-color: var(--border-hover-strong);
}

/* 焦点环 — Phase 1R §5.5 / Phase 2 §5.2 */
.ui-popover__trigger:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
  box-shadow: 0 0 0 var(--size-focus-halo) var(--focus-glow);
}

/* 浮层容器 — Phase 1R §6.4 / §5.1 E2（reka-ui 浮层被 Portal 传送，需用 :deep 命中） */
:deep(.ui-popover__content) {
  min-width: calc(var(--space-12) * 4 + var(--space-2));
  padding: var(--size-menu-padding);
  background: var(--surface-overlay);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-md);
  box-shadow:
    inset 0 1px 0 0 var(--surface-highlight),
    var(--shadow-md);
  font-family: var(--font-ui);
  color: var(--text-primary);
  animation: ui-popover-enter var(--duration-fast) var(--ease-decelerate);
}

.ui-popover__title {
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-body);
  color: var(--text-primary);
  padding: var(--space-1) var(--space-2) var(--space-2);
  border-bottom: var(--hairline) solid var(--border-subtle);
  margin-bottom: var(--space-2);
}

.ui-popover__body {
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-body);
  color: var(--text-secondary);
  padding: 0 var(--space-2);
}

/* 箭头 — 底色 --surface-raised / 描边 --border-strong */
:deep(.ui-popover__arrow) {
  fill: var(--surface-raised);
  stroke: var(--border-strong);
  stroke-width: var(--border-1);
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

@keyframes ui-popover-enter {
  from {
    opacity: 0;
    transform: scale(var(--scale-menu-enter));
  }

  to {
    opacity: 1;
    transform: scale(1);
  }
}

/* 减少动效 — Phase 2 §5.4 #4：仅 opacity 80ms，无 scale */
@media (prefers-reduced-motion: reduce) {
  :deep(.ui-popover__content) {
    animation: ui-popover-enter-instant var(--duration-instant) linear;
  }

  @keyframes ui-popover-enter-instant {
    from {
      opacity: 0;
    }

    to {
      opacity: 1;
    }
  }
}
</style>
