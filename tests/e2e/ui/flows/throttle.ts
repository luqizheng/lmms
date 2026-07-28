// notification-service 对 create/publish 公告限流 5 次/60s（common-ratelimit），
// 全量套件一轮 6+ 次发布会触发 429。套件单 worker 串行，用模块级时间戳
// 保证任意两次「发布」点击间隔 ≥13s。
let lastPublishAt = 0;

export async function throttlePublish(): Promise<void> {
  const wait = 13000 - (Date.now() - lastPublishAt);
  if (wait > 0) await new Promise((r) => setTimeout(r, wait));
  lastPublishAt = Date.now();
}
