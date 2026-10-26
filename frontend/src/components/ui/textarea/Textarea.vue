<script setup lang="ts">
import { computed, inject } from "vue";
import { cn } from "@/lib/cn";
import { FORM_FIELD_ERROR_ID } from "../form-field/context";

interface Props {
  modelValue?: string;
  placeholder?: string;
  disabled?: boolean;
  /** 错误态：danger 描边 + 光晕，并输出 aria-invalid */
  invalid?: boolean;
  rows?: number;
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: "",
  placeholder: undefined,
  disabled: false,
  invalid: false,
  rows: 4
});

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

// 位于 FormField 内时自动关联错误文案
const fieldErrorId = inject(FORM_FIELD_ERROR_ID, undefined);

const isInvalid = computed(() => props.invalid || Boolean(fieldErrorId?.value));
const describedBy = computed(() => fieldErrorId?.value);

const classes = computed(() =>
  cn("ui-textarea", { "is-invalid": isInvalid.value })
);

function onInput(event: Event) {
  emit("update:modelValue", (event.target as HTMLTextAreaElement).value);
}
</script>

<template>
  <textarea
    :value="modelValue"
    :placeholder="placeholder"
    :rows="rows"
    :disabled="disabled"
    :aria-invalid="isInvalid ? 'true' : undefined"
    :aria-describedby="describedBy"
    :class="classes"
    @input="onInput"
  />
</template>

<style scoped>
/* Phase 1R §6.2 */
.ui-textarea {
  width: 100%;
  min-height: var(--size-control-md);
  padding: var(--space-2) var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-body);
  color: var(--text-primary);
  background: var(--surface-raised);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-sm);
  resize: vertical;
  transition:
    border-color var(--duration-fast) var(--ease-decelerate),
    box-shadow var(--duration-fast) var(--ease-decelerate);
}

.ui-textarea::placeholder {
  color: var(--text-tertiary);
}

.ui-textarea:hover:not(:disabled):not(.is-invalid) {
  border-color: var(--border-strong);
}

.ui-textarea:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
  border-color: var(--accent);
  box-shadow: 0 0 0 var(--size-input-halo) var(--input-focus-glow);
}

.ui-textarea.is-invalid {
  border-color: var(--danger);
  box-shadow: 0 0 0 var(--size-input-halo) var(--danger-focus-glow);
}

.ui-textarea.is-invalid:focus-visible {
  outline-color: var(--danger);
  box-shadow: 0 0 0 var(--size-input-halo) var(--danger-focus-glow);
}

.ui-textarea:disabled {
  background: var(--surface-base);
  border-color: var(--border-subtle);
  color: var(--text-disabled);
  cursor: not-allowed;
}
</style>
