import { resolve } from "node:path";

/** 构建根目录 */
export const root = process.cwd();

export function pathResolve(...paths: string[]) {
  return resolve(root, ...paths);
}

/** 路径别名 */
export const alias = {
  "@": pathResolve("src"),
  "@build": pathResolve("build")
};

/**
 * 把 .env 里的字符串转成对应的类型。
 * "true"/"false" → 布尔，纯数字 → 数字，其余保持字符串。
 */
export function wrapperEnv(
  envConf: Record<string, string>
): Record<string, any> {
  const ret: Record<string, any> = {};

  for (const [key, value] of Object.entries(envConf)) {
    if (value === "true") ret[key] = true;
    else if (value === "false") ret[key] = false;
    else if (/^\d+$/.test(value)) ret[key] = Number(value);
    else ret[key] = value;
  }

  return ret;
}
