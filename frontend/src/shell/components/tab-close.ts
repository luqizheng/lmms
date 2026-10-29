/**
 * 标签栏批量关闭的纯计算，供 AppShell 的单标签与批量关闭共用。
 */

/** 标签栏条目（与 TabBar.vue 的 TabItem 结构一致） */
export interface TabItem {
  path: string;
  title: string;
  closable?: boolean;
}

/**
 * 计算按 `shouldClose` 批量关闭后的剩余标签与回退跳转目标。
 *
 * 仅当激活标签（activePath 对应项）被关时才给回退目标：从其原位置
 * 先向左、再向右找最近的幸存标签——与单标签 closeTab 的回退顺序一致。
 * 激活标签没被关就返回 fallback = undefined，调用方不跳转。
 */
export function planTabClose(
  tabs: TabItem[],
  shouldClose: (tab: TabItem, index: number) => boolean,
  activePath?: string
): { remaining: TabItem[]; fallback?: TabItem } {
  const activeIndex = tabs.findIndex(tab => tab.path === activePath);
  const activeClosed =
    activeIndex !== -1 && shouldClose(tabs[activeIndex], activeIndex);
  let fallback: TabItem | undefined;
  if (activeClosed) {
    for (let i = activeIndex - 1; i >= 0 && !fallback; i -= 1) {
      if (!shouldClose(tabs[i], i)) fallback = tabs[i];
    }
    for (let i = activeIndex + 1; i < tabs.length && !fallback; i += 1) {
      if (!shouldClose(tabs[i], i)) fallback = tabs[i];
    }
  }
  return {
    remaining: tabs.filter((tab, index) => !shouldClose(tab, index)),
    fallback
  };
}
