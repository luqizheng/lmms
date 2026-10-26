<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { MenuNode } from "../store/menu";
import SidebarMenuItem from "./SidebarMenuItem.vue";

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

const { t } = useI18n();

/** 展开状态：记录哪些分组是展开的 */
const expanded = defineModel<Set<string>>("expanded", {
  default: () => new Set<string>()
});
</script>

<template>
  <nav
    class="ui-sidebar"
    :class="{ 'is-collapsed': props.collapsed }"
    :aria-label="t('shell.nav.label')"
  >
    <ul class="ui-sidebar__list">
      <SidebarMenuItem
        v-for="node in props.menus"
        :key="node.id"
        v-model:expanded="expanded"
        :node="node"
        :active-path="props.activePath"
        :depth="0"
        @select="emit('select', $event)"
      />
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

/* 收起态：只留图标 */
.ui-sidebar.is-collapsed .ui-sidebar__list {
  padding: var(--space-4) var(--space-2);
}

.ui-sidebar.is-collapsed :deep(.ui-sidebar__item) {
  justify-content: center;
  padding: 0;
}

.ui-sidebar.is-collapsed :deep(.ui-sidebar__label),
.ui-sidebar.is-collapsed :deep(.ui-sidebar__chevron) {
  display: none;
}
</style>
