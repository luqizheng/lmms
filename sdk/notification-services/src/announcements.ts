// 公告管理（notification-service，经网关 /v1/notification/**，需要鉴权）。
// 读接口面向当前用户（服务端从 JWT 取身份）；create/publish/revoke 需管理员权限（403）。
// WS 推送（/v1/notification/ws）不在本 SDK 范围。

import type { HttpCore } from "./http.js";
import type {
  Announcement,
  CreateAnnouncement,
  ListMineQuery,
  PublishResult,
  ReadResult,
  RevokeResult,
} from "./types.js";

export class NotificationAnnouncementsApi {
  constructor(private readonly http: HttpCore) {}

  /** 当前用户的公告列表：GET /v1/notification/announcements（裸数组，无分页）。 */
  listMine(query: ListMineQuery = {}): Promise<Announcement[]> {
    return this.http.get<Announcement[]>("/v1/notification/announcements", { params: query, auth: true });
  }

  /** 创建公告（管理员）：POST /v1/notification/announcements → 201 {id}。 */
  create(request: CreateAnnouncement): Promise<{ id: number }> {
    return this.http.post<{ id: number }>("/v1/notification/announcements", request, { auth: true });
  }

  /** 公告详情：GET /v1/notification/announcements/{id}。 */
  get(id: number): Promise<Announcement> {
    return this.http.get<Announcement>(`/v1/notification/announcements/${id}`, { auth: true });
  }

  /** 发布公告（管理员）：POST .../publish → {id,status,user_count}。 */
  publish(id: number): Promise<PublishResult> {
    return this.http.post<PublishResult>(`/v1/notification/announcements/${id}/publish`, undefined, { auth: true });
  }

  /** 撤回公告（管理员）：POST .../revoke → {id,status}。 */
  revoke(id: number): Promise<RevokeResult> {
    return this.http.post<RevokeResult>(`/v1/notification/announcements/${id}/revoke`, undefined, { auth: true });
  }

  /** 标记已读：POST .../read → {ok,changed}。 */
  markRead(id: number): Promise<ReadResult> {
    return this.http.post<ReadResult>(`/v1/notification/announcements/${id}/read`, undefined, { auth: true });
  }
}

export class NotificationUnreadApi {
  constructor(private readonly http: HttpCore) {}

  /** 当前用户未读数：GET /v1/notification/unread-count → {count}。 */
  count(): Promise<{ count: number }> {
    return this.http.get<{ count: number }>("/v1/notification/unread-count", { auth: true });
  }
}
