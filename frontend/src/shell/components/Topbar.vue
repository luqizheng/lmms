<script setup lang="ts">
import { Bell, RefreshCw, Search } from "lucide-vue-next";
import { Avatar } from "@/components/ui/avatar";
import UserMenu from "./UserMenu.vue";
import { useI18n } from "vue-i18n";

const { t } = useI18n();

interface Crumb {
  label: string;
  to?: string;
}

interface Props {
  breadcrumbs?: Crumb[];
  unread?: number;
  user?: { name: string; email?: string; avatar?: string; roles?: string[] };
  searchPlaceholder?: string;
}

const props = withDefaults(defineProps<Props>(), {
  breadcrumbs: () => [],
  unread: 0,
  user: undefined,
  searchPlaceholder: undefined
});

const emit = defineEmits<{
  search: [];
  refresh: [];
  notifications: [];
  profile: [];
  logout: [];
}>();
</script>

<template>
  <header class="ui-topbar">
    <nav class="ui-topbar__crumbs" :aria-label="t('shell.nav.breadcrumb')">
      <ol class="ui-topbar__crumb-list">
        <li
          v-for="(crumb, index) in props.breadcrumbs"
          :key="index"
          class="ui-topbar__crumb"
        >
          <a
            v-if="crumb.to && index !== props.breadcrumbs.length - 1"
            :href="crumb.to"
          >
            {{ crumb.label }}
          </a>
          <span
            v-else
            :aria-current="
              index === props.breadcrumbs.length - 1 ? 'page' : undefined
            "
          >
            {{ crumb.label }}
          </span>
        </li>
      </ol>
    </nav>

    <button type="button" class="ui-topbar__search" @click="emit('search')">
      <Search class="ui-topbar__search-icon" :size="16" aria-hidden="true" />
      <span class="ui-topbar__search-placeholder">{{
        props.searchPlaceholder ?? t("shell.search.placeholder")
      }}</span>
      <kbd class="ui-topbar__kbd">⌘K</kbd>
    </button>

    <div class="ui-topbar__actions">
      <button
        type="button"
        class="ui-topbar__icon-button"
        :aria-label="t('shell.actions.refresh')"
        @click="emit('refresh')"
      >
        <RefreshCw :size="18" aria-hidden="true" />
      </button>

      <button
        type="button"
        class="ui-topbar__icon-button ui-topbar__bell"
        :aria-label="t('shell.actions.notifications')"
        @click="emit('notifications')"
      >
        <Bell :size="18" aria-hidden="true" />
        <span
          v-if="props.unread > 0"
          class="ui-topbar__dot"
          aria-hidden="true"
        />
      </button>

      <UserMenu
        :user="props.user"
        @profile="emit('profile')"
        @logout="emit('logout')"
      />
    </div>
  </header>
</template>

<style scoped>
/* 顶栏 — Phase 1R §4.3：高 56px */
.ui-topbar {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  height: var(--size-topbar);
  padding: 0 var(--space-6);
  background: var(--surface-base);
  border-bottom: 1px solid var(--border-default);
}

.ui-topbar__crumbs {
  flex: 1;
  min-width: 0;
  overflow: hidden;
}

.ui-topbar__crumb-list {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  color: var(--text-secondary);
}

.ui-topbar__crumb a {
  color: var(--text-secondary);
  text-decoration: none;
}

.ui-topbar__crumb a:hover {
  color: var(--text-primary);
}

.ui-topbar__search {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  width: 320px;
  height: 32px;
  padding: 0 var(--space-2);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--surface-raised);
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  color: var(--text-tertiary);
  cursor: pointer;
  transition: border-color var(--duration-fast) var(--ease-decelerate);
}

.ui-topbar__search:hover {
  border-color: var(--border-strong);
}

.ui-topbar__search:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}

.ui-topbar__search-icon {
  flex: none;
  color: var(--text-tertiary);
}

.ui-topbar__search-placeholder {
  flex: 1;
  text-align: left;
}

/* 键帽 — Phase 1R §5.7 */
.ui-topbar__kbd {
  padding: 0 var(--space-1);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-xs);
  background: var(--surface-base);
  box-shadow: inset 0 -1px 0 var(--border-strong);
  font-family: var(--font-mono);
  font-size: var(--font-size-kbd);
  color: var(--text-secondary);
}

.ui-topbar__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.ui-topbar__icon-button {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

.ui-topbar__icon-button:hover {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-topbar__icon-button:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}

/* 铃铛上的 accent 微点 —— 全屏唯一的彩色焦点 */
.ui-topbar__dot {
  position: absolute;
  top: 8px;
  right: 8px;
  width: 6px;
  height: 6px;
  border-radius: var(--radius-full);
  background: var(--accent);
}

@media (max-width: 1024px) {
  .ui-topbar {
    padding: 0 var(--space-4);
  }

  .ui-topbar__search {
    width: 36px;
    justify-content: center;
    padding: 0;
  }

  .ui-topbar__search-placeholder,
  .ui-topbar__kbd {
    display: none;
  }
}
</style>
