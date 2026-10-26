<script setup lang="ts">
export interface BreadcrumbItem {
  label: string;
  to?: string;
}

interface Props {
  /** 层级条目 */
  items: BreadcrumbItem[];
  /** 导航的无障碍名称 */
  ariaLabel?: string;
}

withDefaults(defineProps<Props>(), { ariaLabel: "面包屑" });
</script>

<template>
  <nav class="ui-breadcrumb" :aria-label="ariaLabel">
    <ol class="ui-breadcrumb__list">
      <li
        v-for="(item, index) in items"
        :key="index"
        class="ui-breadcrumb__item"
        :aria-current="index === items.length - 1 ? 'page' : undefined"
      >
        <a
          v-if="item.to && index !== items.length - 1"
          class="ui-breadcrumb__link"
          :href="item.to"
        >
          {{ item.label }}
        </a>
        <span v-else class="ui-breadcrumb__current">{{ item.label }}</span>
        <span
          v-if="index < items.length - 1"
          class="ui-breadcrumb__separator"
          aria-hidden="true"
        >
          /
        </span>
      </li>
    </ol>
  </nav>
</template>

<style scoped>
/* 面包屑 — Phase 2 §3.2：前层级 secondary 可点击，当前页 primary */
.ui-breadcrumb__list {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ui-breadcrumb__item {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-small);
}

.ui-breadcrumb__link {
  color: var(--text-secondary);
  text-decoration: none;
}

.ui-breadcrumb__link:hover {
  color: var(--text-primary);
}

.ui-breadcrumb__link:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
  border-radius: var(--radius-xs);
}

.ui-breadcrumb__current {
  color: var(--text-primary);
}

.ui-breadcrumb__separator {
  color: var(--text-tertiary);
}
</style>
