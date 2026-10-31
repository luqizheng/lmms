// prue_backend DTO 镜像（动态路由 + 菜单配置版本）。字段名以后端 serde 序列化为唯一事实源：
//   - services/prue_backend/src/dto/menu_config.rs
// 全部 JSON 响应包 {code,message,data} 信封（见 envelope.ts），SDK 方法返回解包后的 data。

// ---------- 动态路由（GET /v1/pure/get-async-routes） ----------

export interface RouteNodeMeta {
  title?: string;
  icon?: string;
  showLink?: boolean;
  rank?: number;
  /** 路由级权限码（服务端校验为字符串数组） */
  auths?: string[];
  roles?: string[];
  keepAlive?: boolean;
  hiddenTag?: boolean;
}

/**
 * 动态路由节点。服务端只做弱校验（path 必填且全局唯一、name 可选且唯一、
 * meta.roles/auths 若存在必须为字符串数组），data 是发布的 routes_json 原样内容。
 */
export interface RouteNode {
  path: string;
  name?: string;
  component?: string;
  redirect?: string;
  meta?: RouteNodeMeta;
  children?: RouteNode[];
}

// ---------- 菜单配置版本（/v1/pure/menu-configs*） ----------

export interface MenuConfigSummary {
  id: string;
  version: number;
  name: string;
  description: string | null;
  status: string;
  is_active: boolean;
  created_by: string | null;
  created_at: string;
  updated_at: string;
  published_at: string | null;
  /** 配置内容指纹，更新时作为 if_match_etag 传回做乐观锁 */
  routes_etag: string;
}

export interface MenuConfigDetail extends MenuConfigSummary {
  routes_json: unknown;
}

export interface MenuConfigListResponse {
  items: MenuConfigSummary[];
  total: number;
}

export interface CreateMenuConfigRequest {
  name: string;
  description?: string;
  /** 必须是数组（服务端校验），如 RouteNode[] */
  routes_json: unknown;
  created_by?: string;
}

export interface UpdateMenuConfigRequest {
  name?: string;
  description?: string;
  routes_json?: unknown;
  /** 乐观锁：传读取时的 routes_etag；不匹配返回 409（error.body.data.current_etag 为当前值） */
  if_match_etag?: string;
}
