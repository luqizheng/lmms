import { inject } from "vue";

export interface ToastOptions {
  variant?: "success" | "info" | "warning" | "danger";
  title: string;
  description?: string;
  closable?: boolean;
  closeLabel?: string;
  duration?: number;
}

/**
 * Toast 调用入口。
 *
 * 页面通过 `const { toast } = useToast()` 触发提示；
 * ToastHost 尚未挂载时降级为 no-op（不抛错，便于单测与 SSR）。
 */
export interface ToastApi {
  toast(options: ToastOptions): void;
  dismiss(): void;
}

export const TOAST_HOST_KEY = Symbol("prism.toast.host");

const NOOP: ToastApi = { toast: () => {}, dismiss: () => {} };

export function useToast(): ToastApi {
  return inject<ToastApi | undefined>(TOAST_HOST_KEY) ?? NOOP;
}
