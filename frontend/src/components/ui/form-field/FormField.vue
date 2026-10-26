<script setup lang="ts">
import { computed, provide, useId } from "vue";
import { cn } from "@/lib/cn";
import { FORM_FIELD_ERROR_ID } from "./context";

interface Props {
  /** 字段名（传入已翻译文案） */
  label?: string;
  /** 必填：显示装饰性星号 */
  required?: boolean;
  /** 说明文案；与 error 同时存在时以 error 为准 */
  description?: string;
  /** 错误文案；非空即进入错误态并对读屏播报 */
  error?: string;
  /** 关联控件 id */
  htmlFor?: string;
}

const props = withDefaults(defineProps<Props>(), {
  label: undefined,
  required: false,
  description: undefined,
  error: undefined,
  htmlFor: undefined
});

const classes = computed(() =>
  cn("ui-form-field", { "is-invalid": Boolean(props.error) })
);

const errorId = useId();
const activeErrorId = computed(() => (props.error ? errorId : undefined));

// 供 slot 内的控件（Input / Textarea / Select …）自动关联错误信息
provide(FORM_FIELD_ERROR_ID, activeErrorId);
</script>

<template>
  <div :class="classes">
    <label v-if="props.label" class="ui-form-field__label" :for="props.htmlFor">
      <span>{{ props.label }}</span>
      <span
        v-if="props.required"
        class="ui-form-field__required"
        aria-hidden="true"
        >*</span
      >
    </label>
    <slot />
    <p
      v-if="props.error"
      :id="errorId"
      class="ui-form-field__error"
      role="alert"
    >
      {{ props.error }}
    </p>
    <p v-else-if="props.description" class="ui-form-field__description">
      {{ props.description }}
    </p>
  </div>
</template>

<style scoped>
/* Phase 1R §6.2：错误文案 12px/18px danger */
.ui-form-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ui-form-field__label {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-small);
  color: var(--text-primary);
}

.ui-form-field__required {
  color: var(--danger);
}

.ui-form-field__error {
  margin: var(--space-1) 0 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-caption);
  color: var(--danger);
}

.ui-form-field__description {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-caption);
  color: var(--text-tertiary);
}
</style>
