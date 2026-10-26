<script setup lang="ts">
import { computed, useId } from "vue";
import { SwitchRoot, SwitchThumb } from "reka-ui";

interface Props {
  /** 开关状态（受控） */
  modelValue?: boolean;
  /** 禁用整个开关 */
  disabled?: boolean;
  /** 表单必填 */
  required?: boolean;
  /** 表单字段名（reka-ui 在 form 内生成隐藏 input） */
  name?: string;
  /** 触发器 id，便于 label / aria-labelledby 关联 */
  id?: string;
  /** 可见标签文案（由调用方注入 i18n） */
  label?: string;
  /** 标签是否仅对读屏可见 */
  hideLabel?: boolean;
  /** 无障碍名称（未提供 label 时使用，例如纯图标场景） */
  ariaLabel?: string;
  /**
   * 读屏专用状态文案（由调用方注入 i18n）。
   * 组件自身不硬编码中文。
   */
  stateLabel?: { on: string; off: string };
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: false,
  disabled: false,
  required: false,
  name: undefined,
  id: undefined,
  label: undefined,
  hideLabel: false,
  ariaLabel: undefined,
  stateLabel: undefined
});

const emit = defineEmits<{ "update:modelValue": [value: boolean] }>();

const uid = useId();
/** 根按钮 id：未显式传入时自动生成，保证 label 与 aria-describedby 可关联 */
const rootId = computed(() => props.id ?? `ui-switch-${uid}`);
const labelId = computed(() => `ui-switch-label-${uid}`);
const stateId = computed(() => `ui-switch-state-${uid}`);

/** 当前状态对应的读屏文案 */
const stateText = computed(() =>
  props.modelValue ? props.stateLabel?.on : props.stateLabel?.off
);

function onUpdate(value: unknown) {
  emit("update:modelValue", Boolean(value));
}
</script>

<template>
  <div class="ui-switch">
    <!-- 根节点即 role=switch 的按钮：reka-ui 负责 ARIA 语义与状态（Phase 1R §6.5） -->
    <SwitchRoot
      :id="rootId"
      class="ui-switch__root"
      :model-value="modelValue"
      :disabled="disabled"
      :required="required"
      :name="name"
      :aria-label="label ? undefined : ariaLabel"
      :aria-labelledby="label ? labelId : undefined"
      :aria-describedby="stateText ? stateId : undefined"
      @update:model-value="onUpdate"
    >
      <SwitchThumb class="ui-switch__thumb" />
    </SwitchRoot>

    <!-- 状态播报：仅读屏可见，不依赖尚未引入的全局 base.css（Phase 2 §5.3） -->
    <span v-if="stateText" :id="stateId" class="ui-sr-only">{{
      stateText
    }}</span>

    <!-- 可见标签：与根按钮通过 for/id 关联，点击标签即切换 -->
    <label
      v-if="label"
      :id="labelId"
      class="ui-switch__label"
      :class="{ 'ui-sr-only': hideLabel }"
      :for="rootId"
    >
      {{ label }}
    </label>
  </div>
</template>

<style scoped>
.ui-switch {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
}

/* 轨道 — Phase 1R §6.5：44×24px / 圆角 full */
.ui-switch__root {
  position: relative;
  display: inline-flex;
  align-items: center;
  box-sizing: border-box;
  flex: none;
  width: calc(var(--space-6) + var(--space-5));
  height: var(--space-6);
  padding: 0;
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-full);
  background: var(--border-default);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard);
}

/* 开启态：底 --accent */
.ui-switch__root[data-state="checked"] {
  background: var(--accent);
  border-color: var(--accent);
}

/* 禁用态 — 关闭态底 --surface-sunken */
.ui-switch__root[data-disabled] {
  background: var(--surface-sunken);
  border-color: var(--border-subtle);
  cursor: not-allowed;
}

/* 禁用态 — 开启态底 accent 40%（tokens.css 暂无该 token，用 color-mix 从 --accent 推导） */
.ui-switch__root[data-disabled][data-state="checked"] {
  background: color-mix(in srgb, var(--accent) 40%, transparent);
  border-color: transparent;
}

/* 焦点环 — Phase 1R §5.5 / Phase 2 §5.2 */
.ui-switch__root:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
  box-shadow: 0 0 0 var(--size-focus-halo) var(--focus-glow);
}

/* 滑块 — 20px 纯白；左右各留 1px 空隙（44 = 1 描边 + 1 空隙 + 20 + 20 + 1 空隙 + 1 描边） */
.ui-switch__thumb {
  position: absolute;
  top: 50%;
  left: calc(var(--space-1) / 4);
  width: var(--space-5);
  height: var(--space-5);
  border-radius: var(--radius-full);
  background: var(--surface-raised);
  transform: translateY(-50%);
  /* 180ms = --duration-fast(150ms) + --duration-instant(80ms) × 3/8 */
  transition:
    transform calc(var(--duration-fast) + var(--duration-instant) * 3 / 8)
      var(--ease-standard),
    width calc(var(--duration-fast) + var(--duration-instant) * 3 / 8)
      var(--ease-standard);
}

/* 开启后右移 20px（180ms standard） */
.ui-switch__root[data-state="checked"] .ui-switch__thumb {
  transform: translateY(-50%) translateX(var(--space-5));
}

/* 按下时滑块横向拉伸至 24px（右边缘锚定，松开回弹） */
.ui-switch__root:active .ui-switch__thumb {
  width: var(--space-6);
}

.ui-switch__root:active[data-state="checked"] .ui-switch__thumb {
  transform: translateY(-50%) translateX(calc(var(--space-5) - var(--space-1)));
}

/* 禁用态滑块 — --border-strong */
.ui-switch__root[data-disabled] .ui-switch__thumb {
  background: var(--border-strong);
}

.ui-switch__label {
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

/* 减少动效 — Phase 2 §5.4 #7：位移瞬时完成，不做拉伸形变 */
@media (prefers-reduced-motion: reduce) {
  .ui-switch__thumb {
    transition-duration: var(--duration-instant);
  }

  .ui-switch__root:active .ui-switch__thumb {
    width: var(--space-5);
  }

  .ui-switch__root:active[data-state="checked"] .ui-switch__thumb {
    transform: translateY(-50%) translateX(var(--space-5));
  }
}
</style>
