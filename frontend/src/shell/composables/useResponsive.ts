import { computed, onBeforeUnmount, onMounted, ref } from "vue";

/** PRISM Phase 2 §4 的断点 */
export const BREAKPOINTS = {
  sm: 640,
  md: 768,
  lg: 1024,
  xl: 1280
} as const;

export type Breakpoint = keyof typeof BREAKPOINTS;

const COLLAPSED_KEY = "prism_sidebar_collapsed";

/**
 * 响应式状态。
 *
 * 降级链（Phase 2 §4.2）：
 *   侧边栏 隐藏(<640) → 图标栏(≥768) → 完整展开(≥1024)
 *   指标卡 1 → 2 → 4 列
 *   表格 卡片化(<640) → 横向滚动(≥640)
 */
export function useResponsive() {
  const width = ref(typeof window === "undefined" ? 1440 : window.innerWidth);

  function update() {
    width.value = window.innerWidth;
  }

  onMounted(() => {
    update();
    window.addEventListener("resize", update, { passive: true });
  });

  onBeforeUnmount(() => {
    window.removeEventListener("resize", update);
  });

  const atLeast = (breakpoint: Breakpoint) =>
    computed(() => width.value >= BREAKPOINTS[breakpoint]);

  return {
    width,
    isMobile: atLeast("sm"),
    isTablet: atLeast("md"),
    isDesktop: atLeast("lg"),
    isWide: atLeast("xl")
  };
}

/** 侧栏折叠状态（用户偏好，localStorage 持久化） */
export function useSidebarCollapsed() {
  const collapsed = ref(false);

  onMounted(() => {
    try {
      collapsed.value = window.localStorage.getItem(COLLAPSED_KEY) === "1";
    } catch {
      collapsed.value = false;
    }
  });

  function toggle() {
    collapsed.value = !collapsed.value;
    try {
      window.localStorage.setItem(COLLAPSED_KEY, collapsed.value ? "1" : "0");
    } catch {
      // localStorage 不可用时仅保留内存态
    }
  }

  return { collapsed, toggle };
}
