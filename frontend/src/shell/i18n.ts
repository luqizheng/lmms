import { type I18n, createI18n } from "vue-i18n";
import type { App } from "vue";

/**
 * 壳的国际化。
 *
 * 与 legacy 的 `@/plugins/i18n` 共用同一份 `locales/*.yaml`，
 * 但**不混入 element-plus 语言包** —— 新壳不使用 element-plus。
 */
const cache = Object.fromEntries(
  Object.entries(
    import.meta.glob("../../locales/*.{yaml,yml}", { eager: true })
  ).map(([key, value]: any) => {
    const matched = key.match(/([A-Za-z0-9-_]+)\./i)![1];
    return [matched, value.default];
  })
);

export const i18n: I18n = createI18n({
  legacy: false,
  locale: "zh",
  fallbackLocale: "en",
  messages: {
    zh: cache["zh-CN"] ?? {},
    en: cache["en"] ?? {}
  }
});

export function useI18n(app: App) {
  app.use(i18n);
}
