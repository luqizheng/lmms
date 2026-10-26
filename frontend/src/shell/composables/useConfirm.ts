import { inject } from "vue";

export interface ConfirmOptions {
  /** 确认框标题 */
  title: string;
  /** 说明文案 */
  description?: string;
  /** 确认按钮文案 */
  confirmLabel?: string;
  /** 取消按钮文案 */
  cancelLabel?: string;
  /** 危险操作：确认按钮用 danger 语义 */
  danger?: boolean;
}

export interface ConfirmApi {
  /** 弹出确认框，用户确认返回 true，取消返回 false */
  confirm(options: ConfirmOptions): Promise<boolean>;
}

export const CONFIRM_HOST_KEY = Symbol("prism.confirm.host");

const FALLBACK: ConfirmApi = {
  confirm: async () => false
};

/**
 * 确认框调用入口。
 *
 * 页面用 `const { confirm } = useConfirm()` 拿到 Promise 式的确认框；
 * ConfirmHost 未挂载时降级为「取消」，不抛错。
 */
export function useConfirm(): ConfirmApi {
  return inject<ConfirmApi | undefined>(CONFIRM_HOST_KEY) ?? FALLBACK;
}
