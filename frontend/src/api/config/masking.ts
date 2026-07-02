// 敏感配置项掩码工具
// 后端约定: ConfigItem.is_secret = true 时 value 应被掩码显示。
// 后端缺口 (2026-07-01): 当前 GET /config/item/{id} 仍返回明文,
//   CFG_CONFIG_REVEAL 权限未挂钩 API 层 —— UI 掩码仅为 UX,
//   调用方仍可通过 API 读到明文。后端补 reveal 检查后,
//   前端无须改 helper, 仅切换是否调用 maskValue 即可。

const MASK = "********";

export function maskSecretValue(value: string | null | undefined): string {
  if (value == null) return "";
  if (value.length === 0) return "";
  return MASK;
}

export function isMaskedDisplay(item: { is_secret?: boolean | null }): boolean {
  return Boolean(item.is_secret);
}

export function shouldMaskInList(item: {
  is_secret?: boolean | null;
}): boolean {
  return isMaskedDisplay(item);
}

// 给 view 用的 reactive state 容器:
//   view 在 readOnly 字段显示时调用 toDisplayValue,
//   reveal 按钮触发 reveal 时 push id 到 revealed Set,
//   Set 内的 id 显示明文（前提是当前用户持有 CFG_CONFIG_REVEAL 权限,
//   由 v-perms 指令保护按钮可见性）。
import { ref } from "vue";
const revealedIds = ref<Set<string>>(new Set());

export function isRevealed(id: string): boolean {
  return revealedIds.value.has(id);
}

export function reveal(id: string): void {
  revealedIds.value.add(id);
  revealedIds.value = new Set(revealedIds.value);
}

export function hide(id: string): void {
  revealedIds.value.delete(id);
  revealedIds.value = new Set(revealedIds.value);
}

export function toDisplayValue(
  id: string,
  rawValue: string | null | undefined,
  isSecret: boolean | null | undefined
): string {
  if (!isSecret) return rawValue ?? "";
  if (isRevealed(id)) return rawValue ?? "";
  return maskSecretValue(rawValue);
}
