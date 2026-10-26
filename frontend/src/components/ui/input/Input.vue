<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

interface Props {
  modelValue?: string | number;
  type?: string;
  placeholder?: string;
  disabled?: boolean;
  /** 错误态：danger 描边 + 焦点光晕 */
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

const classes = computed(() =>
  cn("ui-input", `ui-input--${props.size}`, { "is-invalid": props.invalid })
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
    :class="classes"
    @input="onInput"
  />
</template>

<style scoped>
/* Phase 1R §6.2 */
.ui-input {
  width: 100%;
  height: 36px;
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

.ui-input:focus-visible {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--input-focus-glow);
}

.ui-input.is-invalid {
  border-color: var(--danger);
}

.ui-input.is-invalid:focus-visible {
  box-shadow: 0 0 0 3px var(--danger-focus-glow);
}

.ui-input:disabled {
  background: var(--surface-base);
  border-color: var(--border-subtle);
  color: var(--text-disabled);
  cursor: not-allowed;
}

.ui-input--sm {
  height: 32px;
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

.ui-input--lg {
  height: 40px;
}
</style>
