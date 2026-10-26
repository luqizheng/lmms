<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, watch } from "vue";
import { RouterView, useRoute, useRouter } from "vue-router";
import { Menu as MenuIcon, PanelLeft } from "lucide-vue-next";
import { Sheet } from "@/components/ui/sheet";
import { Command, type CommandItem } from "@/components/ui/command";
import { ToastHost } from "@/components/ui/toast";
import ConfirmHost from "./ConfirmHost.vue";
import Sidebar from "./Sidebar.vue";
import Topbar from "./Topbar.vue";
import TabBar, { type TabItem } from "./TabBar.vue";
import { useAuthStore } from "../store/auth";
import { useMenuStore } from "../store/menu";
import {
  useResponsive,
  useSidebarCollapsed
} from "../composables/useResponsive";
import { useI18n } from "vue-i18n";

const { t } = useI18n();

const route = useRoute();
const router = useRouter();
const auth = useAuthStore();
const menu = useMenuStore();

const { isTablet, isDesktop } = useResponsive();
const { collapsed, toggle } = useSidebarCollapsed();

const drawerOpen = ref(false);
const mainRef = ref<HTMLElement | null>(null);
const liveMessage = ref("");
const commandOpen = ref(false);
const tabs = ref<TabItem[]>([]);

const activePath = computed(() => route.path);

const breadcrumbs = computed(() => {
  const crumbs: Array<{ label: string; to?: string }> = [];
  const node = menu.findByPath(route.path);
  if (node) crumbs.push({ label: node.name });
  const title = (route.meta?.title as string | undefined) ?? "";
  if (title && (!node || node.name !== title)) crumbs.push({ label: title });
  return crumbs;
});

const user = computed(() =>
  auth.user
    ? {
        name: auth.user.nickname || auth.user.username,
        email: auth.user.email ?? undefined,
        avatar: auth.user.avatar ?? undefined,
        roles: auth.user.roles
      }
    : undefined
);

/** 命令面板条目：菜单路径 + 静态动作 */
const commandItems = computed<CommandItem[]>(() => {
  const items: CommandItem[] = [];
  const walk = (nodes: typeof menu.menus) => {
    for (const node of nodes) {
      if (node.path)
        items.push({
          id: node.path,
          label: node.name,
          group: t("shell.nav.group")
        });
      if (node.children.length) walk(node.children);
    }
  };
  walk(menu.menus);
  return items;
});

function onCommandSelect(id: string) {
  commandOpen.value = false;
  if (id.startsWith("/")) router.push(id);
}

function onLogout() {
  auth.signOut().finally(() => router.push({ name: "login" }));
}

function openCommand() {
  commandOpen.value = true;
}

function onKeydown(event: KeyboardEvent) {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
    event.preventDefault();
    openCommand();
  }
}

onMounted(() => window.addEventListener("keydown", onKeydown));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));

// 路由切换：焦点移到主内容 + 礼貌区播报新页标题（Phase 2 §5.2）
watch(
  () => route.fullPath,
  () => {
    const title = (route.meta?.title as string | undefined) ?? "";
    liveMessage.value = title;
    mainRef.value?.focus();
  }
);

// 打开过的页面进标签栏
watch(
  () => route.path,
  path => {
    if (!path || path === "/login" || path === "/") return;
    const title = (route.meta?.title as string | undefined) ?? path;
    if (!tabs.value.some(tab => tab.path === path)) {
      tabs.value = [...tabs.value, { path, title }];
    }
  },
  { immediate: true }
);

function closeTab(path: string) {
  const index = tabs.value.findIndex(tab => tab.path === path);
  tabs.value = tabs.value.filter(tab => tab.path !== path);
  if (path === route.path) {
    const next = tabs.value[index - 1] ?? tabs.value[index];
    if (next) router.push(next.path);
  }
}
</script>

<template>
  <div class="ui-app-shell">
    <a class="skip-link" href="#main-content">{{ t("shell.skipToContent") }}</a>

    <!-- 桌面侧栏：≥768 常驻 -->
    <div v-if="isTablet" class="ui-app-shell__sidebar">
      <Sidebar
        :menus="menu.menus"
        :collapsed="isDesktop ? collapsed : true"
        :active-path="activePath"
        @select="path => router.push(path)"
      />
    </div>

    <!-- 移动端抽屉侧栏：<640 -->
    <Sheet
      v-model:open="drawerOpen"
      :title="t('shell.drawer.title')"
      side="left"
    >
      <Sidebar
        :menus="menu.menus"
        :active-path="activePath"
        @select="
          path => {
            drawerOpen = false;
            router.push(path);
          }
        "
      />
    </Sheet>

    <div class="ui-app-shell__body">
      <Topbar
        :breadcrumbs="breadcrumbs"
        :user="user"
        @search="openCommand"
        @refresh="router.go(0)"
        @notifications="router.push('/notification')"
        @profile="router.push('/profile')"
        @logout="onLogout"
      />

      <!-- 折叠开关：仅在桌面 -->
      <button
        v-if="isDesktop"
        type="button"
        class="ui-app-shell__collapse"
        :aria-label="
          collapsed
            ? t('shell.actions.expandSidebar')
            : t('shell.actions.collapseSidebar')
        "
        @click="toggle"
      >
        <PanelLeft :size="18" aria-hidden="true" />
      </button>

      <!-- 多标签页栏：桌面专属 -->
      <TabBar
        v-if="isTablet"
        class="ui-app-shell__tabs"
        :tabs="tabs"
        :active-path="activePath"
        @select="path => router.push(path)"
        @close="closeTab"
      />

      <main
        id="main-content"
        ref="mainRef"
        class="ui-app-shell__main"
        tabindex="-1"
      >
        <RouterView />
      </main>
      <p class="ui-sr-only" role="status" aria-live="polite">
        {{ liveMessage }}
      </p>
    </div>

    <!-- 移动端顶栏的菜单按钮 -->
    <button
      v-if="!isTablet"
      type="button"
      class="ui-app-shell__drawer-button"
      :aria-label="t('shell.actions.openNavigation')"
      @click="drawerOpen = true"
    >
      <MenuIcon :size="18" aria-hidden="true" />
    </button>

    <Command
      v-model:open="commandOpen"
      :items="commandItems"
      @select="onCommandSelect"
    />

    <!-- 全局提示宿主：页面通过 useToast() 触发 -->
    <ToastHost />

    <!-- 全局确认框宿主：页面通过 useConfirm() 触发 -->
    <ConfirmHost />
  </div>
</template>

<style scoped>
.ui-app-shell {
  display: flex;
  min-height: 100vh;
  background: var(--surface-base);
}

.ui-app-shell__sidebar {
  flex: none;
  height: 100vh;
  position: sticky;
  top: 0;
}

.ui-app-shell__body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.ui-app-shell__main {
  flex: 1;
  padding: var(--space-6);
  outline: none;
}

.ui-app-shell__collapse {
  position: absolute;
  top: calc(var(--size-topbar) / 2 - 18px);
  left: calc(var(--size-sidebar) - 44px);
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
    left var(--duration-medium) var(--ease-decelerate);
}

.ui-app-shell__collapse:hover {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-app-shell__collapse:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}

.ui-app-shell__drawer-button {
  position: fixed;
  top: var(--space-3);
  left: var(--space-3);
  z-index: 20;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border: 0;
  border-radius: var(--radius-sm);
  background: var(--surface-raised);
  color: var(--text-secondary);
  cursor: pointer;
}

.ui-app-shell__drawer-button:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}

@media (max-width: 640px) {
  .ui-app-shell__main {
    padding: var(--space-4);
  }
}
</style>
