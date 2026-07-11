/**
 * 把后端相对路径(`/v1/file/<id>`)拼上 `VITE_API_BASE`,给 `<img src>` / `ElImage.src`
 * 这种不走 axios baseURL 的渲染场景用。后端故意返回相对路径以适配多环境
 * (`services/file-service/src/routes/file.rs`);axios 会自动加 baseURL,但 `<img>` 不会。
 * 绝对 URL(`http(s)://…`,如 presign 返回)原样返回。
 *
 * 故意放在独立文件(`api/file/file-url.ts`),不依赖 `@/utils/http`,
 * 让单元测试无需走完整 http→router import chain。
 */
export function fileUrl(path?: string | null): string {
  if (!path) return "";
  const trimmed = path.trim();
  if (!trimmed) return "";
  if (/^https?:\/\//i.test(trimmed)) return trimmed;

  const base = (import.meta.env.VITE_API_BASE as string | undefined) ?? "/api";
  const prefix = base.endsWith("/") ? base.slice(0, -1) : base;
  const suffix = trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
  return `${prefix}${suffix}`;
}
