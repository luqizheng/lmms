import { http } from "@/utils/http";

export type ScopeType = "user" | "role" | "dept";
export type AnnouncementStatus = "draft" | "scheduled" | "published" | "revoked";

export interface Announcement {
  id: number;
  title: string;
  body: string;
  publish_at: string;
  expire_at: string | null;
  pinned: boolean;
  popover: boolean;
  status: AnnouncementStatus;
  created_by: number;
  created_at: string;
}

export interface CreateAnnouncementDto {
  title: string;
  body: string;
  publish_at: string;
  expire_at?: string | null;
  pinned?: boolean;
  popover?: boolean;
  recipients: { scope_type: ScopeType; scope_id: number }[];
}

export function listAnnouncements() {
  return http.request<Announcement[]>("get", "/v1/notification/announcements");
}
export function getAnnouncement(id: number) {
  return http.request<Announcement>(
    "get",
    `/v1/notification/announcements/${id}`
  );
}
export function createAnnouncement(data: CreateAnnouncementDto) {
  return http.request<{ id: number }>(
    "post",
    "/v1/notification/announcements",
    { data }
  );
}
export function publishAnnouncement(id: number) {
  return http.request<unknown>(
    "post",
    `/v1/notification/announcements/${id}/publish`
  );
}
export function revokeAnnouncement(id: number) {
  return http.request<unknown>(
    "post",
    `/v1/notification/announcements/${id}/revoke`
  );
}
export function markRead(id: number) {
  return http.request<{ ok: boolean; changed: boolean }>(
    "post",
    `/v1/notification/announcements/${id}/read`
  );
}
export function unreadCount() {
  return http.request<{ count: number }>(
    "get",
    "/v1/notification/unread-count"
  );
}
