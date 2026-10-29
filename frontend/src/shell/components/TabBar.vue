<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { ChevronLeft, ChevronRight, X } from "lucide-vue-next";
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

const scrollRef = ref<HTMLElement | null>(null);
const canScrollLeft = ref(false);
const canScrollRight = ref(false);

function updateScrollState() {
  const el = scrollRef.value;
  if (!el) return;
  const max = el.scrollWidth - el.clientWidth;
  canScrollLeft.value = el.scrollLeft > 1;
  canScrollRight.value = max > 1 && el.scrollLeft < max - 1;
}

/** 按一屏宽度翻页；平滑滚动，滚动事件会刷新按钮状态 */
function scrollByPage(direction: -1 | 1) {
  const el = scrollRef.value;
  if (!el) return;
  const page = Math.max(el.clientWidth - 24, 80);
  if (typeof el.scrollBy === "function") {
    el.scrollBy({ left: direction * page, behavior: "smooth" });
  } else {
    el.scrollLeft += direction * page;
  }
}

function ensureActiveVisible() {
  const el = scrollRef.value;
  if (!el) return;
  const active = el.querySelector<HTMLElement>(".ui-tabbar__tab.is-current");
  active?.scrollIntoView?.({ block: "nearest", inline: "nearest" });
  updateScrollState();
}

watch(
  () => props.activePath,
  async () => {
    await nextTick();
    ensureActiveVisible();
  }
);

watch(
  () => props.tabs.length,
  async () => {
    await nextTick();
    updateScrollState();
  }
);

let resizeObserver: ResizeObserver | undefined;

onMounted(() => {
  updateScrollState();
  if (scrollRef.value && typeof ResizeObserver !== "undefined") {
    resizeObserver = new ResizeObserver(updateScrollState);
    resizeObserver.observe(scrollRef.value);
  }
  ensureActiveVisible();
});

onBeforeUnmount(() => resizeObserver?.disconnect());
</script>

<template>
  <div class="ui-tabbar">
    <button
      type="button"
      class="ui-tabbar__nav"
      :disabled="!canScrollLeft"
      :aria-label="t('shell.tabs.scrollPrev')"
      @click="scrollByPage(-1)"
    >
      <ChevronLeft :size="16" aria-hidden="true" />
    </button>

    <div
      ref="scrollRef"
      class="ui-tabbar__scroll"
      role="tablist"
      :aria-label="t('shell.tabs.label')"
      @scroll="updateScrollState"
    >
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

    <button
      type="button"
      class="ui-tabbar__nav"
      :disabled="!canScrollRight"
      :aria-label="t('shell.tabs.scrollNext')"
      @click="scrollByPage(1)"
    >
      <ChevronRight :size="16" aria-hidden="true" />
    </button>
  </div>
</template>

<style scoped>
/* 多标签页栏 — 设计文档 §5：当前项 accent-subtle 底 + accent-deep 字 + 底缘 2px accent 指示条 */
.ui-tabbar {
  display: flex;
  gap: var(--space-1);
  align-items: stretch;
  height: 36px;
  padding: 0 var(--space-1);
  background: var(--surface-raised);
  border-bottom: 1px solid var(--border-subtle);
}

/* 滚动容器：隐藏原生滚动条，避免压住标签内容 */
.ui-tabbar__scroll {
  display: flex;
  flex: 1;
  gap: var(--space-1);
  align-items: stretch;
  min-width: 0;
  padding: 0 var(--space-2);
  overflow-x: auto;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.ui-tabbar__scroll::-webkit-scrollbar {
  display: none;
}

.ui-tabbar__nav {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 28px;
  padding: 0;
  color: var(--text-secondary);
  cursor: pointer;
  background: transparent;
  border: 0;
  border-radius: var(--radius-xs);
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

.ui-tabbar__nav:hover:not(:disabled) {
  color: var(--text-primary);
  background: var(--surface-hover-veil);
}

.ui-tabbar__nav:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: calc(var(--size-focus-ring) * -1);
}

.ui-tabbar__nav:disabled {
  cursor: default;
  opacity: 0.35;
}

.ui-tabbar__tab {
  position: relative;
  display: inline-flex;
  gap: var(--space-2);
  align-items: center;
  height: 36px;
  padding: 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  color: var(--text-secondary);
  white-space: nowrap;
  cursor: pointer;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm) var(--radius-sm) 0 0;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

.ui-tabbar__tab:hover {
  color: var(--text-primary);
  background: var(--surface-hover-veil);
}

.ui-tabbar__tab:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: calc(var(--size-focus-ring) * -1);
}

.ui-tabbar__tab.is-current {
  color: var(--accent-deep);
  background: var(--accent-subtle);
}

.ui-tabbar__tab.is-current::after {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: var(--size-indicator);
  content: "";
  background: var(--accent);
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
  color: var(--text-tertiary);
  cursor: pointer;
  background: transparent;
  border: 0;
  border-radius: var(--radius-xs);
}

.ui-tabbar__close:hover {
  color: var(--text-primary);
  background: var(--surface-active-veil);
}

.ui-tabbar__close:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}
</style>
