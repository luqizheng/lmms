<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

interface Props {
  modelValue?: string;
  placeholder?: string;
  disabled?: boolean;
  /** 错误态：danger 描边 + 焦点光晕 */
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

const classes = computed(() =>
  cn("ui-textarea", { "is-invalid": props.invalid })
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
    :class="classes"
    @input="onInput"
  />
</template>

<style scoped>
/* Phase 1R §6.2 */
.ui-textarea {
  width: 100%;
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
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--input-focus-glow);
}

.ui-textarea.is-invalid {
  border-color: var(--danger);
}

.ui-textarea.is-invalid:focus-visible {
  box-shadow: 0 0 0 3px var(--danger-focus-glow);
}

.ui-textarea:disabled {
  background: var(--surface-base);
  border-color: var(--border-subtle);
  color: var(--text-disabled);
  cursor: not-allowed;
}
</style>
