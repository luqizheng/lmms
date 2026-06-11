export const DEBUG = process.env.E2E_DEBUG === '1';

export function logReq(method: string, path: string, data?: unknown) {
  if (!DEBUG) return;
  console.log(`>> ${method} ${path}`, data ?? '');
}

export function logRes(status: number, body: unknown) {
  if (!DEBUG) return;
  console.log(`<< ${status}`, body);
}
