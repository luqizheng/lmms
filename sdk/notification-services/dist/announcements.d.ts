import type { HttpCore } from "./http.js";
import type { Announcement, CreateAnnouncement, ListMineQuery, PublishResult, ReadResult, RevokeResult } from "./types.js";
export declare class NotificationAnnouncementsApi {
    private readonly http;
    constructor(http: HttpCore);
    /** 当前用户的公告列表：GET /v1/notification/announcements（裸数组，无分页）。 */
    listMine(query?: ListMineQuery): Promise<Announcement[]>;
    /** 创建公告（管理员）：POST /v1/notification/announcements → 201 {id}。 */
    create(request: CreateAnnouncement): Promise<{
        id: number;
    }>;
    /** 公告详情：GET /v1/notification/announcements/{id}。 */
    get(id: number): Promise<Announcement>;
    /** 发布公告（管理员）：POST .../publish → {id,status,user_count}。 */
    publish(id: number): Promise<PublishResult>;
    /** 撤回公告（管理员）：POST .../revoke → {id,status}。 */
    revoke(id: number): Promise<RevokeResult>;
    /** 标记已读：POST .../read → {ok,changed}。 */
    markRead(id: number): Promise<ReadResult>;
}
export declare class NotificationUnreadApi {
    private readonly http;
    constructor(http: HttpCore);
    /** 当前用户未读数：GET /v1/notification/unread-count → {count}。 */
    count(): Promise<{
        count: number;
    }>;
}
//# sourceMappingURL=announcements.d.ts.map