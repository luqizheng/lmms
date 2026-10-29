<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch
} from "vue";
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

const emit = defineEmits<{
  select: [path: string];
  close: [path: string];
  closeLeft: [path: string];
  closeRight: [path: string];
  closeOthers: [path: string];
  closeAll: [];
}>();

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

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  closeContextMenu();
});

/* ── 右键上下文菜单：TabBar 管交互与禁用态，关闭语义由 AppShell 裁决 ── */

type ContextAction =
  | "close"
  | "closeLeft"
  | "closeRight"
  | "closeOthers"
  | "closeAll";

const menuOpen = ref(false);
const menuPos = ref({ x: 0, y: 0 });
const menuPath = ref("");
const menuEl = ref<HTMLElement | null>(null);
let menuTrigger: HTMLElement | null = null;

const targetIndex = computed(() =>
  props.tabs.findIndex(tab => tab.path === menuPath.value)
);

const menuItems = computed(() => {
  const index = targetIndex.value;
  const hasClosableBefore = props.tabs.some(
    (tab, i) => i < index && tab.closable !== false
  );
  const hasClosableAfter = props.tabs.some(
    (tab, i) => i > index && tab.closable !== false
  );
  const hasClosableOthers = props.tabs.some(
    (tab, i) => i !== index && tab.closable !== false
  );
  const hasClosableAny = props.tabs.some(tab => tab.closable !== false);
  return [
    {
      action: "close" as const,
      label: t("shell.tabs.close"),
      disabled: props.tabs[index]?.closable === false
    },
    {
      action: "closeLeft" as const,
      label: t("shell.tabs.closeLeft"),
      disabled: !hasClosableBefore
    },
    {
      action: "closeRight" as const,
      label: t("shell.tabs.closeRight"),
      disabled: !hasClosableAfter
    },
    {
      action: "closeOthers" as const,
      label: t("shell.tabs.closeOthers"),
      disabled: !hasClosableOthers
    },
    {
      action: "closeAll" as const,
      label: t("shell.tabs.closeAll"),
      disabled: !hasClosableAny
    }
  ];
});

function openContextMenu(event: MouseEvent, tab: TabItem) {
  menuPath.value = tab.path;
  menuPos.value = { x: event.clientX, y: event.clientY };
  menuTrigger =
    event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
  menuOpen.value = true;
  document.addEventListener("pointerdown", onDocPointerDown, true);
  document.addEventListener("focusin", onDocFocusIn);
  window.addEventListener("keydown", onMenuKeydown);
  window.addEventListener("scroll", closeContextMenu, true);
  window.addEventListener("resize", closeContextMenu);
  void nextTick(() => {
    const el = menuEl.value;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    menuPos.value = {
      x: Math.max(
        8,
        Math.min(menuPos.value.x, window.innerWidth - rect.width - 8)
      ),
      y: Math.max(
        8,
        Math.min(menuPos.value.y, window.innerHeight - rect.height - 8)
      )
    };
    el.querySelector<HTMLElement>("button:not(:disabled)")?.focus();
  });
}

function closeContextMenu() {
  if (!menuOpen.value) return;
  menuOpen.value = false;
  document.removeEventListener("pointerdown", onDocPointerDown, true);
  document.removeEventListener("focusin", onDocFocusIn);
  window.removeEventListener("keydown", onMenuKeydown);
  window.removeEventListener("scroll", closeContextMenu, true);
  window.removeEventListener("resize", closeContextMenu);
  menuTrigger?.focus();
  menuTrigger = null;
}

function onDocPointerDown(event: PointerEvent) {
  const target = event.target;
  if (!(target instanceof Node)) return;
  if (!menuEl.value?.contains(target)) closeContextMenu();
}

function onDocFocusIn(event: FocusEvent) {
  const target = event.target;
  if (!(target instanceof Node)) return;
  if (!menuEl.value?.contains(target)) closeContextMenu();
}

function onMenuKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    closeContextMenu();
    return;
  }
  if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
  event.preventDefault();
  const items = Array.from(
    menuEl.value?.querySelectorAll<HTMLElement>("button:not(:disabled)") ?? []
  );
  if (!items.length) return;
  const current = items.indexOf(document.activeElement as HTMLElement);
  const step = event.key === "ArrowDown" ? 1 : items.length - 1;
  items[(current + step) % items.length]?.focus();
}

function runContextMenuAction(action: ContextAction) {
  const path = menuPath.value;
  closeContextMenu();
  if (action === "close") emit("close", path);
  else if (action === "closeLeft") emit("closeLeft", path);
  else if (action === "closeRight") emit("closeRight", path);
  else if (action === "closeOthers") emit("closeOthers", path);
  else emit("closeAll");
}
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
        @contextmenu.prevent="openContextMenu($event, tab)"
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

    <!-- Teleport 到 body：fixed 定位才能逃出 overflow 滚动容器的裁剪 -->
    <Teleport to="body">
      <div
        v-if="menuOpen"
        ref="menuEl"
        class="ui-tabbar__menu"
        role="menu"
        :style="{ left: `${menuPos.x}px`, top: `${menuPos.y}px` }"
        @contextmenu.prevent
      >
        <button
          v-for="item in menuItems"
          :key="item.action"
          type="button"
          role="menuitem"
          class="ui-tabbar__menu-item"
          :disabled="item.disabled"
          @click="runContextMenuAction(item.action)"
        >
          {{ item.label }}
        </button>
      </div>
    </Teleport>
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

.ui-tabbar__menu {
  position: fixed;
  z-index: 50;
  display: flex;
  min-width: 140px;
  flex-direction: column;
  padding: var(--space-1);
  background: var(--surface-raised);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-md);
}

.ui-tabbar__menu-item {
  padding: var(--space-2) var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  color: var(--text-primary);
  text-align: left;
  cursor: pointer;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
}

.ui-tabbar__menu-item:hover:not(:disabled) {
  background: var(--surface-hover-veil);
}

.ui-tabbar__menu-item:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: calc(var(--size-focus-ring) * -1);
}

.ui-tabbar__menu-item:focus-visible:not(:disabled) {
  background: var(--surface-hover-veil);
}

.ui-tabbar__menu-item:disabled {
  color: var(--text-tertiary);
  cursor: default;
}
</style>
