import { type NotificationServicesOptions } from "./http.js";
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
export declare function createNotificationServices(options?: NotificationServicesOptions): NotificationServices;
//# sourceMappingURL=index.d.ts.map