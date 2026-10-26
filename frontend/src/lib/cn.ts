import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * 合并 class 名的统一入口。
 *
 * - clsx 处理条件 / 数组 / 对象写法
 * - tailwind-merge 消解 Tailwind 工具类冲突（后写的覆盖先写的）
 */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}

export type { ClassValue };
