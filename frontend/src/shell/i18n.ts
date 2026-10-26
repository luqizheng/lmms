import { type I18n, createI18n } from "vue-i18n";
import type { App } from "vue";

/**
 * 壳的国际化。
 *
 * 文案按 **命名空间分文件**：`locales/<locale>/<namespace>.yaml`。
 * 一个命名空间 = 一个文件，业务模块各占各的文件（`locales/<locale>/pm/<module>.yaml`），
 * 并行开发时不会互相覆盖。
 *
 * 不混入 element-plus 语言包 —— 新壳不使用 element-plus。
 */
const files = import.meta.glob("../../locales/**/*.yaml", { eager: true }) as Record<
  string,
  { default: Record<string, unknown> }
>;

function buildMessages() {
  const messages: Record<string, Record<string, unknown>> = {};

  for (const [path, mod] of Object.entries(files)) {
    // ../../locales/<locale>/<namespace>.yaml
    const parts = path.replace(/^\.\.\/\.\.\//, "").split("/");
    // parts = ["locales", <locale>, ...<namespace path>]
    const locale = parts[1];
    // 命名空间 = 相对路径（pm/bugs.yaml → "pm.bugs"）
    const namespace = parts
      .slice(2)
      .join(".")
      .replace(/\.ya?ml$/, "");

    messages[locale] = messages[locale] ?? {};

    // 文件内容可能带着命名空间自身的键（如 shell.yaml 里是 `shell:`），
    // 这种情况要解包一层，否则 key 会变成 shell.shell.xxx
    const content = mod.default;
    const keys = Object.keys(content);
    const segments = namespace.split(".");
    const lastSegment = segments[segments.length - 1];
    const unwrapped =
      keys.length === 1 && keys[0] === lastSegment
        ? (content[lastSegment] as Record<string, unknown>)
        : content;

    // vue-i18n 的 key 是点号路径，命名空间必须落成**嵌套**结构：
    // locales/zh-CN/pm/bugs.yaml → messages["zh-CN"]["pm"]["bugs"]
    let node = messages[locale];
    for (const segment of segments.slice(0, -1)) {
      node[segment] = (node[segment] as Record<string, unknown>) ?? {};
      node = node[segment] as Record<string, unknown>;
    }
    node[lastSegment] = {
      ...(node[lastSegment] as Record<string, unknown> | undefined),
      ...unwrapped
    };
  }

  return messages;
}

export const i18n: I18n = createI18n({
  legacy: false,
  locale: "zh-CN",
  fallbackLocale: "en",
  messages: buildMessages()
});

export function useI18n(app: App) {
  app.use(i18n);
}
