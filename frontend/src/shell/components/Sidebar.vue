<script setup lang="ts">
import { computed } from "vue";
import { ChevronRight } from "lucide-vue-next";
import type { MenuNode } from "../store/menu";
import { useI18n } from "vue-i18n";

const { t } = useI18n();

interface Props {
  /** 菜单树 */
  menus: MenuNode[];
  /** 收起态（图标栏） */
  collapsed?: boolean;
  /** 当前路径 */
  activePath?: string;
}

const props = withDefaults(defineProps<Props>(), {
  collapsed: false,
  activePath: ""
});

const emit = defineEmits<{ select: [path: string] }>();

/** 展开状态：记录哪些分组是展开的 */
const expanded = defineModel<Set<string>>("expanded", {
  default: () => new Set<string>()
});

function isActive(node: MenuNode): boolean {
  return node.path === props.activePath;
}

function hasActiveChild(node: MenuNode): boolean {
  return node.children.some(child => isActive(child) || hasActiveChild(child));
}

function toggle(node: MenuNode) {
  if (node.children.length === 0) {
    if (node.path) emit("select", node.path);
    return;
  }
  const next = new Set(expanded.value);
  if (next.has(node.id)) next.delete(node.id);
  else next.add(node.id);
  expanded.value = next;
}

const isExpanded = (node: MenuNode) =>
  expanded.value.has(node.id) || hasActiveChild(node);

/** 菜单名可能是 i18n key，也可能是原文 —— 交给上层解析，这里原样展示 */
const label = computed(() => props.menus);
</script>

<template>
  <nav
    class="ui-sidebar"
    :class="{ 'is-collapsed': props.collapsed }"
    :aria-label="t('shell.nav.label')"
  >
    <ul class="ui-sidebar__list">
      <li
        v-for="node in label"
        :key="node.id"
        class="ui-sidebar__group"
        :class="{ 'is-active': isActive(node) || hasActiveChild(node) }"
      >
        <button
          type="button"
          class="ui-sidebar__item"
          :class="{ 'is-current': isActive(node) }"
          :aria-current="isActive(node) ? 'page' : undefined"
          :title="props.collapsed ? node.name : undefined"
          @click="toggle(node)"
        >
          <span v-if="node.icon" class="ui-sidebar__icon" aria-hidden="true">{{
            node.icon
          }}</span>
          <span v-if="!props.collapsed" class="ui-sidebar__label">{{
            node.name
          }}</span>
          <ChevronRight
            v-if="!props.collapsed && node.children.length"
            class="ui-sidebar__chevron"
            :class="{ 'is-open': isExpanded(node) }"
            :size="16"
            aria-hidden="true"
          />
        </button>

        <ul
          v-if="!props.collapsed && node.children.length && isExpanded(node)"
          class="ui-sidebar__sublist"
        >
          <li v-for="child in node.children" :key="child.id">
            <button
              type="button"
              class="ui-sidebar__item ui-sidebar__item--child"
              :class="{ 'is-current': isActive(child) }"
              :aria-current="isActive(child) ? 'page' : undefined"
              @click="toggle(child)"
            >
              <span
                v-if="child.icon"
                class="ui-sidebar__icon"
                aria-hidden="true"
                >{{ child.icon }}</span
              >
              <span class="ui-sidebar__label">{{ child.name }}</span>
            </button>
          </li>
        </ul>
      </li>
    </ul>
  </nav>
</template>

<style scoped>
/* 侧栏 — Phase 1R §6.9：白底侧栏浮在暖灰画布上 */
.ui-sidebar {
  width: var(--size-sidebar);
  height: 100%;
  background: var(--surface-raised);
  border-right: 1px solid var(--border-default);
  overflow-y: auto;
  transition: width var(--duration-medium) var(--ease-decelerate);
}

.ui-sidebar.is-collapsed {
  width: var(--size-sidebar-collapsed);
}

.ui-sidebar__list {
  margin: 0;
  padding: var(--space-4) var(--space-3);
  list-style: none;
}

.ui-sidebar__group + .ui-sidebar__group {
  margin-top: var(--space-6);
}

.ui-sidebar__sublist {
  margin: var(--space-1) 0 0;
  padding: 0;
  list-style: none;
}

.ui-sidebar__item {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  height: 36px;
  padding: 0 var(--space-3);
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  color: var(--text-secondary);
  text-align: left;
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

.ui-sidebar__item:hover {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-sidebar__item:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}

.ui-sidebar__item.is-current {
  background: var(--accent-subtle);
  color: var(--accent-deep);
}

.ui-sidebar__item.is-current::before {
  position: absolute;
  left: 0;
  top: 50%;
  width: 2px;
  height: 20px;
  transform: translateY(-50%);
  background: var(--accent);
  border-radius: var(--radius-full);
  content: "";
}

.ui-sidebar__item--child {
  padding-left: var(--space-8);
}

.ui-sidebar__icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
  flex: none;
  font-size: var(--font-size-small);
  color: inherit;
}

.ui-sidebar__item.is-current .ui-sidebar__icon {
  color: var(--accent);
}

.ui-sidebar__label {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ui-sidebar__chevron {
  flex: none;
  color: var(--text-tertiary);
  transition: transform var(--duration-fast) var(--ease-decelerate);
}

.ui-sidebar__chevron.is-open {
  transform: rotate(90deg);
}

/* 收起态：只留图标 */
.ui-sidebar.is-collapsed .ui-sidebar__list {
  padding: var(--space-4) var(--space-2);
}

.ui-sidebar.is-collapsed .ui-sidebar__item {
  justify-content: center;
  padding: 0;
}
</style>
