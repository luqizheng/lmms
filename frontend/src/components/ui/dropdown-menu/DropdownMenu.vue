<script setup lang="ts">
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuTrigger
} from "reka-ui";
import { cn } from "@/lib/cn";
import type { DropdownMenuItemOption } from "./types";

interface Props {
  /** 菜单项列表 */
  items?: DropdownMenuItemOption[];
  /** 触发器的无障碍名称 */
  ariaLabel?: string;
  /** 禁用整个菜单 */
  disabled?: boolean;
  /** 浮层与触发器的对齐方式 */
  align?: "start" | "center" | "end";
  /** 浮层展开方向 */
  side?: "top" | "bottom";
  /** 模态模式：浮层打开时锁定背景交互（默认开） */
  modal?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  items: () => [],
  ariaLabel: undefined,
  disabled: false,
  align: "start",
  side: "bottom",
  modal: true
});

const emit = defineEmits<{ select: [item: DropdownMenuItemOption] }>();

function onSelect(item: DropdownMenuItemOption) {
  emit("select", item);
}
</script>

<template>
  <DropdownMenuRoot :modal="modal">
    <!-- 触发器：Phase 1R §6.1 Secondary 规格 -->
    <DropdownMenuTrigger
      class="ui-dropdown-menu__trigger"
      :class="cn({ 'is-disabled': disabled })"
      :disabled="disabled"
      :aria-label="ariaLabel"
    >
      <slot />
      <svg
        class="ui-dropdown-menu__chevron"
        viewBox="0 0 16 16"
        focusable="false"
        aria-hidden="true"
      >
        <path
          d="M4 6.5 8 10.5l4-4"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
    </DropdownMenuTrigger>

    <!-- 浮层：Phase 1R §6.4（白底 + border-strong + shadow-md / radius-md / 内边距 6px / 最小宽 200px） -->
    <DropdownMenuPortal>
      <DropdownMenuContent
        class="ui-dropdown-menu__content"
        :side="side"
        :align="align"
        :side-offset="4"
      >
        <DropdownMenuItem
          v-for="item in items"
          :key="item.value ?? item.label"
          class="ui-dropdown-menu__item"
          :class="{
            'ui-dropdown-menu__item--danger': item.danger,
            'is-disabled': item.disabled
          }"
          :disabled="item.disabled"
          @select="onSelect(item)"
        >
          <slot name="item" :item="item">{{ item.label }}</slot>
          <!-- 危险态不能只靠颜色表达：补一段仅读屏可见的提示（WCAG 1.4.1） -->
          <span v-if="item.danger && item.dangerLabel" class="ui-sr-only">
            {{ item.dangerLabel }}
          </span>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>

<style scoped>
/* 触发器 — Phase 1R §6.1 Secondary 规格 */
.ui-dropdown-menu__trigger {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-body);
  color: var(--text-primary);
  background: var(--surface-raised);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    border-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate),
    box-shadow var(--duration-fast) var(--ease-decelerate);
}

.ui-dropdown-menu__trigger:hover:not(:disabled) {
  background: var(--surface-hover-veil);
  border-color: var(--border-hover-strong);
}

.ui-dropdown-menu__trigger:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
  box-shadow: 0 0 0 var(--size-focus-halo) var(--focus-glow);
}

.ui-dropdown-menu__trigger:disabled {
  background: var(--surface-disabled);
  border-color: transparent;
  color: var(--text-disabled);
  cursor: not-allowed;
}

.ui-dropdown-menu__chevron {
  flex: none;
  width: 1em;
  height: 1em;
  color: var(--text-secondary);
}

/* 浮层容器 — Phase 1R §6.4 / §5.1 E2 */
:deep(.ui-dropdown-menu__content) {
  min-width: calc(var(--space-12) * 4 + var(--space-2));
  padding: calc((var(--space-1) + var(--space-2)) / 2);
  background: var(--surface-overlay);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md);
  box-shadow:
    inset 0 1px 0 0 var(--surface-highlight),
    var(--shadow-md);
  animation: ui-dropdown-menu-enter var(--duration-fast) var(--ease-decelerate);
}

/* 菜单项 — Phase 1R §6.4：高 32px / radius-xs / 文字 secondary */
:deep(.ui-dropdown-menu__item) {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--size-control-sm);
  padding: 0 var(--space-2);
  border-radius: var(--radius-xs);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-secondary);
  cursor: pointer;
  user-select: none;
  outline: none;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

/* hover 加灰、字升 primary（§5.3 / §6.4） */
:deep(.ui-dropdown-menu__item[data-highlighted]) {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

:deep(.ui-dropdown-menu__item[data-disabled]) {
  color: var(--text-disabled);
  cursor: not-allowed;
}

/* 危险项：danger 色（删除等不可逆操作） */
:deep(.ui-dropdown-menu__item--danger) {
  color: var(--danger);
}

:deep(.ui-dropdown-menu__item--danger[data-highlighted]) {
  background: var(--danger-subtle);
  color: var(--danger-strong);
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

@keyframes ui-dropdown-menu-enter {
  from {
    opacity: 0;
    transform: scale(0.96);
  }

  to {
    opacity: 1;
    transform: scale(1);
  }
}

/* 减少动效 — Phase 2 §5.4 #4：仅 opacity 80ms，无 scale */
@media (prefers-reduced-motion: reduce) {
  :deep(.ui-dropdown-menu__content) {
    animation: ui-dropdown-menu-enter-instant var(--duration-instant) linear;
  }

  @keyframes ui-dropdown-menu-enter-instant {
    from {
      opacity: 0;
    }

    to {
      opacity: 1;
    }
  }
}
</style>
