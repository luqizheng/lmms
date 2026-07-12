// File upload edge limits for the frontend.
// 后端硬限 (services/file-service/src/config.rs:33-34) = 100 MB。
// 前端在 el-upload :before-upload 必须 reject > MAX_SIZE 的文件，避免无意义上传。
// MIME / 文件名清理是前端 UX 守卫，后端不强制。

export const FILE_MAX_SIZE_BYTES = 100 * 1024 * 1024;

export const FILE_MAX_SIZE_MB = 100;

export const FILE_SIZE_LIMIT_ERROR = `文件大小超过 ${FILE_MAX_SIZE_MB}MB 上限`;

// High-risk executable extensions to block client-side.
// 后端不拒绝，但浏览器侧阻止降低误传。
export const FILE_BLOCKED_EXTENSIONS = [
  ".exe",
  ".bat",
  ".cmd",
  ".sh",
  ".ps1",
  ".msi",
  ".scr",
  ".com",
  ".vbs",
  ".js"
] as const;

export function checkFileExtension(name: string): boolean {
  const lower = name.toLowerCase();
  return !FILE_BLOCKED_EXTENSIONS.some(ext => lower.endsWith(ext));
}

export function checkFileSize(bytes: number): boolean {
  return bytes <= FILE_MAX_SIZE_BYTES;
}
