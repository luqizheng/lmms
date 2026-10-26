<script setup lang="ts">
import { computed } from "vue";
import {
  SelectContent,
  SelectIcon,
  SelectItem,
  SelectItemText,
  SelectPortal,
  SelectRoot,
  SelectTrigger,
  SelectValue,
  SelectViewport
} from "reka-ui";
import { cn } from "@/lib/cn";
import type { SelectOption } from "./types";

interface Props {
  /** 当前选中值（受控） */
  modelValue?: string;
  /** 选项列表 */
  options?: SelectOption[];
  /** 允许清空：选中项可被清除并回到空值 */
  clearable?: boolean;
  /** 清空按钮的无障碍名称 */
  clearLabel?: string;
  /** 未选中时的占位文案（由调用方注入 i18n） */
  placeholder?: string;
  /** 禁用整个控件 */
  disabled?: boolean;
  /** 错误态：输出 aria-invalid（Phase 2 §6.3） */
  invalid?: boolean;
  /** 表单必填 */
  required?: boolean;
  /** 表单字段名（reka-ui 会生成隐藏 input） */
  name?: string;
  /** 触发器 id，便于 label / aria-labelledby 关联 */
  id?: string;
  /** 尺寸系 — Phase 1R §6.2：sm 32px / md 36px */
  size?: "sm" | "md";
  /** 触发器的无障碍名称 */
  ariaLabel?: string;
  /** 浮层与触发器的对齐方式 */
  align?: "start" | "center" | "end";
  /** 浮层展开方向 */
  side?: "top" | "bottom";
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: undefined,
  options: () => [],
  clearable: false,
  clearLabel: "清除",
  placeholder: "",
  disabled: false,
  invalid: false,
  required: false,
  name: undefined,
  id: undefined,
  size: "md",
  ariaLabel: undefined,
  align: "start",
  side: "bottom"
});

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const selectedOption = computed(() =>
  props.options.find(option => option.value === props.modelValue)
);

function onClear() {
  emit("update:modelValue", "");
}

function onUpdate(value: unknown) {
  emit("update:modelValue", String(value));
}
</script>

<template>
  <SelectRoot
    :model-value="modelValue"
    :disabled="disabled"
    :required="required"
    :name="name"
    @update:model-value="onUpdate"
  >
    <!-- 触发器：按 Phase 1R §6.2 输入框规格（白底 / border-default / radius-sm / 36px） -->
    <SelectTrigger
      :id="id"
      class="ui-select__trigger"
      :class="cn(`ui-select__trigger--${size}`, { 'is-invalid': invalid })"
      :aria-label="ariaLabel"
      :aria-invalid="invalid ? 'true' : undefined"
    >
      <SelectValue class="ui-select__value" :placeholder="placeholder">
        <template #default="{ selectedLabel }">
          <slot
            name="value"
            :option="selectedOption"
            :selected-label="selectedLabel"
          >
            <template v-if="selectedLabel.length">{{
              selectedLabel.join(", ")
            }}</template>
            <template v-else>{{ placeholder }}</template>
          </slot>
        </template>
      </SelectValue>
      <button
        v-if="clearable && modelValue"
        type="button"
        class="ui-select__clear"
        :aria-label="clearLabel"
        @click.stop="onClear"
      >
        <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">
          <path
            d="M4 4l8 8M12 4l-8 8"
            stroke="currentcolor"
            stroke-width="1.5"
            stroke-linecap="round"
          />
        </svg>
      </button>
      <SelectIcon class="ui-select__icon" aria-hidden="true">
        <slot name="icon">
          <svg
            class="ui-select__chevron"
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
        </slot>
      </SelectIcon>
    </SelectTrigger>

    <!-- 浮层：Phase 1R §6.4（白底 + border-strong + shadow-md / radius-md / 内边距 6px / 最小宽 200px） -->
    <SelectPortal>
      <SelectContent
        class="ui-select__content"
        :side="side"
        :align="align"
        :side-offset="4"
      >
        <SelectViewport class="ui-select__viewport">
          <SelectItem
            v-for="option in options"
            :key="option.value"
            :value="option.value"
            :disabled="option.disabled"
            class="ui-select__item"
            :class="{ 'is-selected': option.value === modelValue }"
          >
            <!-- 选中项左侧勾选图标（纯装饰，不参与读屏；SVG 无文本，避免污染选项文案） -->
            <svg
              class="ui-select__check"
              viewBox="0 0 16 16"
              focusable="false"
              aria-hidden="true"
            >
              <path
                d="M3.5 8.5l3 3 6-6.5"
                fill="none"
                stroke="currentColor"
                stroke-width="1.6"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
            <SelectItemText class="ui-select__item-text">
              <slot name="item" :option="option">{{ option.label }}</slot>
            </SelectItemText>
          </SelectItem>
        </SelectViewport>
      </SelectContent>
    </SelectPortal>
  </SelectRoot>
</template>

<style scoped>
/* 触发器 — Phase 1R §6.2 输入框规格 */
.ui-select__trigger {
  display: inline-flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  width: 100%;
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-body);
  color: var(--text-primary);
  background: var(--surface-raised);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition:
    border-color var(--duration-fast) var(--ease-decelerate),
    box-shadow var(--duration-fast) var(--ease-decelerate);
}

.ui-select__trigger:hover:not([data-disabled]) {
  border-color: var(--border-strong);
}

/* 聚焦：accent 描边 + 略窄于全局 ring 的光晕（§6.2） */
.ui-select__trigger:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
  border-color: var(--accent);
  box-shadow: 0 0 0 var(--size-input-halo) var(--input-focus-glow);
}

.ui-select__trigger[data-disabled] {
  background: var(--surface-base);
  border-color: var(--border-subtle);
  color: var(--text-disabled);
  cursor: not-allowed;
}

/* 错误态：描边与光晕常驻（§6.2 / Phase 2 §6.3） */
.ui-select__trigger.is-invalid {
  border-color: var(--danger);
  box-shadow: 0 0 0 var(--size-input-halo) var(--danger-focus-glow);
}

.ui-select__trigger.is-invalid:focus-visible {
  outline-color: var(--danger);
  box-shadow: 0 0 0 var(--size-input-halo) var(--danger-focus-glow);
}

/* 尺寸系 — Phase 1R §6.2 */
.ui-select__trigger--sm {
  height: var(--size-control-sm);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

.ui-select__value {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 占位文案：--text-tertiary（§6.2） */
.ui-select__value[data-placeholder] {
  color: var(--text-tertiary);
}

.ui-select__clear {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  flex: none;
  padding: 0;
  border: 0;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text-tertiary);
  cursor: pointer;
}

.ui-select__clear:hover {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-select__clear:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}

.ui-select__icon {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  color: var(--text-secondary);
}

.ui-select__chevron {
  width: 1em;
  height: 1em;
}

/* 浮层容器 — Phase 1R §6.4 / §5.1 E2 */
:deep(.ui-select__content) {
  min-width: calc(var(--space-12) * 4 + var(--space-2));
  padding: calc((var(--space-1) + var(--space-2)) / 2);
  background: var(--surface-overlay);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md);
  box-shadow:
    inset 0 1px 0 0 var(--surface-highlight),
    var(--shadow-md);
  animation: ui-select-enter var(--duration-fast) var(--ease-decelerate);
}

:deep(.ui-select__viewport) {
  padding: 0;
}

/* 菜单项 — Phase 1R §6.4：高 32px / radius-xs / 文字 secondary */
.ui-select__item {
  position: relative;
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
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

/* hover 加灰、字升 primary（§5.3 / §6.4） */
.ui-select__item[data-highlighted] {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-select__item[data-disabled] {
  color: var(--text-disabled);
  cursor: not-allowed;
}

/* 选中项：accent-subtle 底 + accent-deep 字（§6.4） */
.ui-select__item.is-selected {
  background: var(--accent-subtle);
  color: var(--accent-deep);
}

.ui-select__item.is-selected[data-highlighted] {
  background: var(--accent-subtle);
  color: var(--accent-deep);
}

.ui-select__check {
  flex: none;
  width: 1em;
  height: 1em;
  color: var(--accent);
  opacity: 0;
}

.ui-select__item.is-selected .ui-select__check {
  opacity: 1;
}

.ui-select__item-text {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@keyframes ui-select-enter {
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
  :deep(.ui-select__content) {
    animation: ui-select-enter-instant var(--duration-instant) linear;
  }

  @keyframes ui-select-enter-instant {
    from {
      opacity: 0;
    }

    to {
      opacity: 1;
    }
  }
}
</style>
