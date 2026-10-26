<script setup lang="ts">
import { computed, inject } from "vue";
import { cn } from "@/lib/cn";
import { FORM_FIELD_ERROR_ID } from "../form-field/context";

interface Props {
  modelValue?: string | number;
  type?: string;
  placeholder?: string;
  disabled?: boolean;
  /** 错误态：danger 描边 + 光晕，并输出 aria-invalid */
  invalid?: boolean;
  size?: "sm" | "md" | "lg";
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: "",
  type: "text",
  placeholder: undefined,
  disabled: false,
  invalid: false,
  size: "md"
});

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

// 位于 FormField 内时自动关联错误文案
const fieldErrorId = inject(FORM_FIELD_ERROR_ID, undefined);

const isInvalid = computed(() => props.invalid || Boolean(fieldErrorId?.value));
const describedBy = computed(() => fieldErrorId?.value);

const classes = computed(() =>
  cn("ui-input", `ui-input--${props.size}`, { "is-invalid": isInvalid.value })
);

function onInput(event: Event) {
  emit("update:modelValue", (event.target as HTMLInputElement).value);
}
</script>

<template>
  <input
    :type="type"
    :value="modelValue"
    :placeholder="placeholder"
    :disabled="disabled"
    :aria-invalid="isInvalid ? 'true' : undefined"
    :aria-describedby="describedBy"
    :class="classes"
    @input="onInput"
  />
</template>

<style scoped>
/* Phase 1R §6.2 */
.ui-input {
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
  transition:
    border-color var(--duration-fast) var(--ease-decelerate),
    box-shadow var(--duration-fast) var(--ease-decelerate);
}

.ui-input::placeholder {
  color: var(--text-tertiary);
}

.ui-input:hover:not(:disabled):not(.is-invalid) {
  border-color: var(--border-strong);
}

/* 聚焦：accent 描边 + 略窄于全局 ring 的光晕（§6.2） */
.ui-input:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
  border-color: var(--accent);
  box-shadow: 0 0 0 var(--size-input-halo) var(--input-focus-glow);
}

/* 错误态：描边与光晕常驻（§6.2 / Phase 2 §6.3） */
.ui-input.is-invalid {
  border-color: var(--danger);
  box-shadow: 0 0 0 var(--size-input-halo) var(--danger-focus-glow);
}

.ui-input.is-invalid:focus-visible {
  outline-color: var(--danger);
  box-shadow: 0 0 0 var(--size-input-halo) var(--danger-focus-glow);
}

.ui-input:disabled {
  background: var(--surface-base);
  border-color: var(--border-subtle);
  color: var(--text-disabled);
  cursor: not-allowed;
}

.ui-input--sm {
  height: var(--size-control-sm);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

.ui-input--lg {
  height: var(--size-control-lg);
}
</style>
