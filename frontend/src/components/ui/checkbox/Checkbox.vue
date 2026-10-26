<script setup lang="ts">
import { computed, useId } from "vue";
import { CheckboxIndicator, CheckboxRoot } from "reka-ui";

interface Props {
  /** 勾选状态（受控）；"indeterminate" 表示半选 */
  modelValue?: boolean | "indeterminate";
  /** 禁用整个勾选框 */
  disabled?: boolean;
  /** 表单必填 */
  required?: boolean;
  /** 表单字段名（reka-ui 在 form 内生成隐藏 input） */
  name?: string;
  /** 提交值 */
  value?: string;
  /** 触发器 id，便于 label / aria-labelledby 关联 */
  id?: string;
  /** 可见标签文案（由调用方注入 i18n） */
  label?: string;
  /** 标签是否仅对读屏可见 */
  hideLabel?: boolean;
  /** 无障碍名称（未提供 label 时使用） */
  ariaLabel?: string;
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: false,
  disabled: false,
  required: false,
  name: undefined,
  value: undefined,
  id: undefined,
  label: undefined,
  hideLabel: false,
  ariaLabel: undefined
});

const emit = defineEmits<{
  "update:modelValue": [value: boolean | "indeterminate"];
}>();

const uid = useId();
/** 根按钮 id：未显式传入时自动生成，保证 label 与 aria-labelledby 可关联 */
const rootId = computed(() => props.id ?? `ui-checkbox-${uid}`);
const labelId = computed(() => `ui-checkbox-label-${uid}`);

/** 当前状态：半选优先 */
const isIndeterminate = computed(() => props.modelValue === "indeterminate");
const isChecked = computed(() => props.modelValue === true);

function onUpdate(value: unknown) {
  emit("update:modelValue", value as boolean | "indeterminate");
}

/**
 * 空格键切换。
 *
 * 根节点是原生 button，浏览器在 keyup 时会原生派发 click；
 * 这里在 keydown 阶段 preventDefault 取消该原生激活，
 * 改为显式派发一次切换，避免浏览器与本组件各切一次。
 */
function onKeydown(event: KeyboardEvent) {
  if (event.key !== " ") return;
  event.preventDefault();
  if (props.disabled) return;
  onUpdate(!isChecked.value);
}
</script>

<template>
  <div class="ui-checkbox">
    <!-- 根节点即 role=checkbox 的原生按钮：reka-ui 负责 ARIA 语义与状态 -->
    <CheckboxRoot
      :id="rootId"
      class="ui-checkbox__root"
      :model-value="modelValue"
      :disabled="disabled"
      :required="required"
      :name="name"
      :value="value"
      :aria-label="label ? undefined : ariaLabel"
      :aria-labelledby="label ? labelId : undefined"
      @update:model-value="onUpdate"
      @keydown="onKeydown"
    >
      <CheckboxIndicator class="ui-checkbox__indicator">
        <!-- 勾号（选中态） -->
        <svg
          class="ui-checkbox__check"
          viewBox="0 0 16 16"
          focusable="false"
          aria-hidden="true"
        >
          <path
            d="M3.5 8.5l3 3 6-6.5"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
        <!-- 横杠（半选态） -->
        <svg
          class="ui-checkbox__dash"
          viewBox="0 0 16 16"
          focusable="false"
          aria-hidden="true"
        >
          <path
            d="M4 8h8"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
          />
        </svg>
      </CheckboxIndicator>
    </CheckboxRoot>

    <!-- 可见标签：与根按钮通过 for/id 关联，点击标签即切换 -->
    <label
      v-if="label"
      :id="labelId"
      class="ui-checkbox__label"
      :class="{ 'ui-sr-only': hideLabel }"
      :for="rootId"
    >
      {{ label }}
    </label>
  </div>
</template>

<style scoped>
.ui-checkbox {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
}

/* 方框 — Phase 1R 控件语言：16px / 1px --border-strong 描边 / 白底 */
.ui-checkbox__root {
  box-sizing: border-box;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  width: var(--space-4);
  height: var(--space-4);
  padding: 0;
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-xs);
  background: var(--surface-raised);
  color: var(--accent-contrast-text);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    border-color var(--duration-fast) var(--ease-decelerate);
}

.ui-checkbox__root:hover:not([data-disabled]) {
  border-color: var(--border-hover-strong);
}

/* 选中态：底 --accent + 白色勾号 */
.ui-checkbox__root[data-state="checked"],
.ui-checkbox__root[data-state="indeterminate"] {
  background: var(--accent);
  border-color: var(--accent);
}

/* 焦点环 — Phase 1R §5.5 / Phase 2 §5.2 */
.ui-checkbox__root:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
  box-shadow: 0 0 0 var(--size-focus-halo) var(--focus-glow);
}

/* 禁用态 */
.ui-checkbox__root[data-disabled] {
  background: var(--surface-disabled);
  border-color: var(--border-subtle);
  color: var(--text-disabled);
  cursor: not-allowed;
}

.ui-checkbox__indicator {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
}

.ui-checkbox__check,
.ui-checkbox__dash {
  width: 100%;
  height: 100%;
}

/* 半选态显示横杠、隐藏勾号 */
.ui-checkbox__indicator[data-state="indeterminate"] .ui-checkbox__check {
  display: none;
}

.ui-checkbox__indicator[data-state="checked"] .ui-checkbox__dash {
  display: none;
}

.ui-checkbox__label {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-body);
  color: var(--text-primary);
  cursor: pointer;
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
</style>
