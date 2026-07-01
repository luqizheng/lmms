// 错误码 → 用户文案 翻译 helper
// 默认 zh-CN；找不到时退回英文 ErrorMessages，最后退回 code 自身。
// 视图侧 catch (err) { message(translateError(err.code)) } 使用。

import { ErrorMessages } from "@/contracts/error-codes";
import { ErrorMessagesZhCN } from "@/contracts/error-codes.zh-CN";

export type Locale = "zh-CN" | "en";

let currentLocale: Locale = "zh-CN";

export function setLocale(locale: Locale): void {
  currentLocale = locale;
}

export function getLocale(): Locale {
  return currentLocale;
}

export function translateError(code: string): string {
  if (currentLocale === "zh-CN") {
    const zh = (ErrorMessagesZhCN as Record<string, string>)[code];
    if (zh) return zh;
  }
  const en = (ErrorMessages as Record<string, string>)[code];
  if (en) return en;
  return code;
}
