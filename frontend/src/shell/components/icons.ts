import type { Component } from "vue";
import {
  Activity,
  Bell,
  Bot,
  Boxes,
  Building2,
  Clock,
  Database,
  FileText,
  FolderTree,
  Gauge,
  KeyRound,
  LayoutDashboard,
  ListTree,
  Lock,
  Monitor,
  ScrollText,
  Server,
  Settings,
  ShieldCheck,
  SlidersHorizontal,
  Users,
  Wrench
} from "lucide-vue-next";

/**
 * 菜单图标映射。
 *
 * 菜单的 icon 字段存的是 Lucide 图标名（如 `layout-dashboard`）。
 * 这里显式登记常用图标 —— 未登记的名字渲染为空图标而不是把字符串当文本显示。
 */
const ICONS: Record<string, Component> = {
  activity: Activity,
  bell: Bell,
  bot: Bot,
  boxes: Boxes,
  building: Building2,
  "building-2": Building2,
  clock: Clock,
  database: Database,
  file: FileText,
  "file-text": FileText,
  folder: FolderTree,
  "folder-tree": FolderTree,
  gauge: Gauge,
  key: KeyRound,
  "key-round": KeyRound,
  "layout-dashboard": LayoutDashboard,
  list: ListTree,
  "list-tree": ListTree,
  lock: Lock,
  monitor: Monitor,
  "scroll-text": ScrollText,
  server: Server,
  settings: Settings,
  shield: ShieldCheck,
  "shield-check": ShieldCheck,
  sliders: SlidersHorizontal,
  "sliders-horizontal": SlidersHorizontal,
  users: Users,
  wrench: Wrench
};

export function resolveIcon(name?: string): Component | undefined {
  if (!name) return undefined;
  return ICONS[name];
}
