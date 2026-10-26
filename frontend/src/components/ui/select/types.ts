/**
 * Select 选项契约（Phase 1R §6.4）。
 *
 * 文案由调用方注入（i18n），组件内不硬编码中文。
 */
export interface SelectOption {
  /** 选项值，同时作为表单提交值 */
  value: string;
  /** 选项展示文案 */
  label: string;
  /** 禁用态：不可选中、不可聚焦 */
  disabled?: boolean;
}
