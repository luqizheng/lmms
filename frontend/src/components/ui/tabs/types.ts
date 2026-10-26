/**
 * Tabs 页签契约。
 *
 * 文案由调用方注入（i18n），组件内不硬编码中文。
 */
export interface TabItem {
  /** 页签值，同时作为面板的关联键 */
  value: string;
  /** 页签展示文案 */
  label: string;
  /** 禁用态：不可聚焦、不可切换 */
  disabled?: boolean;
}
