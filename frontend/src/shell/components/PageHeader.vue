<script setup lang="ts">
import { ChevronLeft } from "lucide-vue-next";

interface Props {
  /** 页面标题 */
  title?: string;
  /** 副标题 */
  subtitle?: string;
  /** 显示返回按钮 */
  back?: boolean;
  /** 返回按钮的无障碍名称 */
  backLabel?: string;
}

const props = withDefaults(defineProps<Props>(), {
  title: undefined,
  subtitle: undefined,
  back: false,
  backLabel: "返回"
});

const emit = defineEmits<{ back: [] }>();
</script>

<template>
  <header class="ui-page-header">
    <div class="ui-page-header__main">
      <button
        v-if="props.back"
        type="button"
        class="ui-page-header__back"
        :aria-label="props.backLabel"
        @click="emit('back')"
      >
        <ChevronLeft :size="18" aria-hidden="true" />
      </button>
      <div class="ui-page-header__text">
        <h1 v-if="props.title" class="ui-page-header__title">{{ props.title }}</h1>
        <p v-if="props.subtitle" class="ui-page-header__subtitle">{{ props.subtitle }}</p>
      </div>
    </div>
    <div v-if="$slots.actions" class="ui-page-header__actions">
      <slot name="actions" />
    </div>
  </header>
</template>

<style scoped>
.ui-page-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
}

.ui-page-header__main {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-width: 0;
}

.ui-page-header__back {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-md);
  height: var(--size-control-md);
  flex: none;
  padding: 0;
  border: 1px solid var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--surface-raised);
  color: var(--text-secondary);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

.ui-page-header__back:hover {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-page-header__back:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}

.ui-page-header__text {
  min-width: 0;
}

.ui-page-header__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h2);
  line-height: var(--line-height-h2);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h2);
  color: var(--text-primary);
}

.ui-page-header__subtitle {
  margin: var(--space-1) 0 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  color: var(--text-tertiary);
}

.ui-page-header__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: none;
}
</style>
