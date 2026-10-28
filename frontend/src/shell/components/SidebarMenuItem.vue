<script setup lang="ts">
import { computed } from "vue";
import { ChevronRight } from "lucide-vue-next";
import type { MenuNode } from "../store/menu";
import { resolveIcon } from "./icons";

interface Props {
  node: MenuNode;
  activePath: string;
  /** 层级缩进 */
  depth?: number;
}

const props = withDefaults(defineProps<Props>(), { depth: 0 });

const emit = defineEmits<{ select: [path: string] }>();

const expanded = defineModel<Set<string>>("expanded", {
  default: () => new Set<string>()
});

const icon = computed(() => resolveIcon(props.node.icon));

const isActive = computed(() => props.node.path === props.activePath);

const hasActiveChild = (node: MenuNode): boolean =>
  node.children.some(
    child => child.path === props.activePath || hasActiveChild(child)
  );

const isOpen = computed(
  () =>
    props.node.children.length > 0 &&
    (expanded.value.has(props.node.id) || hasActiveChild(props.node))
);

function toggle() {
  if (props.node.children.length === 0) {
    if (props.node.path) emit("select", props.node.path);
    return;
  }
  const next = new Set(expanded.value);
  if (next.has(props.node.id)) next.delete(props.node.id);
  else next.add(props.node.id);
  expanded.value = next;
}
</script>

<template>
  <li
    class="ui-sidebar__group"
    :class="{ 'is-group': props.node.children.length > 0 }"
  >
    <button
      type="button"
      class="ui-sidebar__item"
      :class="{ 'is-current': isActive }"
      :style="{
        paddingLeft: `calc(var(--space-3) + ${props.depth} * var(--space-4))`
      }"
      :aria-current="isActive ? 'page' : undefined"
      :aria-label="props.node.name"
      @click="toggle"
    >
      <component
        :is="icon"
        v-if="icon"
        class="ui-sidebar__icon"
        :size="18"
        aria-hidden="true"
      />
      <span class="ui-sidebar__label">{{ props.node.name }}</span>
      <ChevronRight
        v-if="props.node.children.length"
        class="ui-sidebar__chevron"
        :class="{ 'is-open': isOpen }"
        :size="16"
        aria-hidden="true"
      />
    </button>

    <ul v-if="props.node.children.length && isOpen" class="ui-sidebar__sublist">
      <SidebarMenuItem
        v-for="child in props.node.children"
        :key="child.id"
        v-model:expanded="expanded"
        :node="child"
        :active-path="props.activePath"
        :depth="props.depth + 1"
        @select="emit('select', $event)"
      />
    </ul>
  </li>
</template>

<script lang="ts">
export default { name: "SidebarMenuItem" };
</script>

<style scoped>
/* 菜单节奏：同级项紧凑（4px），分组（带子列表的节点）前后各留 20px。
   旧实现对所有兄弟节点一律 24px，项节奏被拉到 60px，且分组之间与同级项
   之间没有区别——间距不再承载分组语义，一级菜单撑出一屏。 */
.ui-sidebar__group + .ui-sidebar__group {
  margin-top: var(--space-1);
}

.ui-sidebar__group + .ui-sidebar__group.is-group,
.ui-sidebar__group.is-group + .ui-sidebar__group {
  margin-top: var(--space-5);
}

.ui-sidebar__sublist {
  margin: var(--space-2) 0 0;
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

.ui-sidebar__icon {
  flex: none;
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
</style>
