import type { InjectionKey, Ref } from "vue";

/**
 * FormField → 控件的错误上下文。
 *
 * FormField 通过 provide 暴露错误文案的 id，控件（Input / Textarea / Select …）
 * 通过 inject 取用，自动获得 aria-describedby 与 aria-invalid。
 */
export const FORM_FIELD_ERROR_ID: InjectionKey<Ref<string | undefined>> =
  Symbol("prism.form-field.error-id");
