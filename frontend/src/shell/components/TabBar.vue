<script setup lang="ts">
import { X } from "lucide-vue-next";
import { useI18n } from "vue-i18n";

export interface TabItem {
  path: string;
  title: string;
  closable?: boolean;
}

const { t } = useI18n();

interface Props {
  tabs: TabItem[];
  activePath?: string;
  closeLabel?: string;
}

const props = withDefaults(defineProps<Props>(), {
  activePath: "",
  closeLabel: undefined
});

const emit = defineEmits<{ select: [path: string]; close: [path: string] }>();
</script>

<template>
  <div class="ui-tabbar" role="tablist" :aria-label="t('shell.tabs.label')">
    <div
      v-for="tab in props.tabs"
      :key="tab.path"
      class="ui-tabbar__tab"
      :class="{ 'is-current': tab.path === props.activePath }"
      role="tab"
      :aria-selected="tab.path === props.activePath ? 'true' : 'false'"
      :tabindex="tab.path === props.activePath ? 0 : -1"
      @click="emit('select', tab.path)"
      @keydown.enter.prevent="emit('select', tab.path)"
    >
      <span class="ui-tabbar__title">{{ tab.title }}</span>
      <button
        v-if="tab.closable !== false"
        type="button"
        class="ui-tabbar__close"
        :aria-label="props.closeLabel ?? t('shell.actions.closeTab')"
        @click.stop="emit('close', tab.path)"
      >
        <X :size="14" aria-hidden="true" />
      </button>
    </div>
  </div>
</template>

<style scoped>
/* 多标签页栏 — 设计文档 §5：当前项 accent-subtle 底 + accent-deep 字 + 底缘 2px accent 指示条 */
.ui-tabbar {
  display: flex;
  align-items: stretch;
  gap: var(--space-1);
  height: 36px;
  padding: 0 var(--space-3);
  background: var(--surface-raised);
  border-bottom: 1px solid var(--border-subtle);
  overflow-x: auto;
}

.ui-tabbar__tab {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  height: 36px;
  padding: 0 var(--space-3);
  border: 0;
  border-radius: var(--radius-sm) var(--radius-sm) 0 0;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  color: var(--text-secondary);
  white-space: nowrap;
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

.ui-tabbar__tab:hover {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-tabbar__tab:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: calc(var(--size-focus-ring) * -1);
}

.ui-tabbar__tab.is-current {
  background: var(--accent-subtle);
  color: var(--accent-deep);
}

.ui-tabbar__tab.is-current::after {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: var(--size-indicator);
  background: var(--accent);
  content: "";
}

.ui-tabbar__title {
  overflow: hidden;
  text-overflow: ellipsis;
}

.ui-tabbar__close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  padding: 0;
  border: 0;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text-tertiary);
  cursor: pointer;
}

.ui-tabbar__close:hover {
  background: var(--surface-active-veil);
  color: var(--text-primary);
}

.ui-tabbar__close:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}
</style>
