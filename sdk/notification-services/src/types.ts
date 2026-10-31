// notification-service DTO 镜像。字段名以后端 serde 序列化为唯一事实源：
//   - services/notification-service/openapi.yaml + src/models.rs
// 成功响应为裸 JSON；错误体为 {error} / {error:"upstream",code,message}。
// 注意不对称：pinned/popover 创建时传 boolean，响应里是 int32（0/1）——照实镜像。

export type AnnouncementStatus = "draft" | "scheduled" | "published" | "revoked";

export interface Announcement {
  id: number;
  title: string;
  body: string;
  publish_at: string;
  /** 响应里是 0/1 整数（非布尔） */
  pinned: number;
  /** 响应里是 0/1 整数（非布尔） */
  popover: number;
  status: AnnouncementStatus;
  created_by: string;
  created_at: string;
  expire_at: string | null;
}

export type ScopeType = "user" | "role" | "dept";

export interface ScopeInput {
  scope_type: ScopeType;
  scope_id: string;
}

export interface CreateAnnouncement {
  title: string;
  body: string;
  publish_at: string;
  pinned: boolean;
  popover: boolean;
  expire_at?: string | null;
  recipients: ScopeInput[];
}

export interface PublishResult {
  id: number;
  status: "published";
  user_count: number;
}

export interface RevokeResult {
  id: number;
  status: "revoked";
}

export interface ReadResult {
  ok: boolean;
  changed: boolean;
}

export interface ListMineQuery {
  pinned?: boolean;
  active?: boolean;
}
