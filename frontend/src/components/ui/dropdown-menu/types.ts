/**
 * DropdownMenu 菜单项契约（Phase 1R §6.4）。
 *
 * 文案由调用方注入（i18n），组件内不硬编码中文。
 */
export interface DropdownMenuItemOption {
  /** 菜单项展示文案 */
  label: string;
  /** 业务标识，随 select 事件回传 */
  value?: string;
  /** 禁用态：不可点击 */
  disabled?: boolean;
  /** 危险项：使用 danger 色（删除等不可逆操作） */
  danger?: boolean;
  /**
   * 危险项的读屏补充文案（如「危险操作」）。
   *
   * 危险态仅靠颜色表达会违反 WCAG 1.4.1，传入该文案后组件会渲染一段
   * 仅读屏可见的提示文本；文案由调用方注入（i18n）。
   */
  dangerLabel?: string;
}
