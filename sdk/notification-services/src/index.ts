// notification-services：lmms notification-service SDK（公告管理 / 未读数；不含 WS）。
// 零运行时依赖，原生 fetch，浏览器 / Node ≥18 通用；面向 api-gateway 的 /v1/notification 公开路径。

import { HttpCore, type NotificationServicesOptions } from "./http.js";
import { NotificationAnnouncementsApi, NotificationUnreadApi } from "./announcements.js";

export * from "./types.js";
export * from "./errors.js";
export { DEFAULT_BASE_URL, HttpCore, parseDispositionFilename, type NotificationServicesOptions, type RequestOptions, type BytesResponse } from "./http.js";
export { NotificationAnnouncementsApi, NotificationUnreadApi } from "./announcements.js";

export interface NotificationServices {
  /** 公告管理（读面向当前用户，写需管理员）。 */
  readonly announcements: NotificationAnnouncementsApi;
  /** 未读数。 */
  readonly unread: NotificationUnreadApi;
}

/** 创建 notification-service 客户端。 */
export function createNotificationServices(options: NotificationServicesOptions = {}): NotificationServices {
  const http = new HttpCore(options);
  return {
    announcements: new NotificationAnnouncementsApi(http),
    unread: new NotificationUnreadApi(http),
  };
}
