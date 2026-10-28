import { http } from "./client";

/** `GET /v1/iam/users/simple` 的单条记录（仅 id + 昵称） */
export interface UserSimple {
  id: string;
  nickname: string;
}

export interface UserSimpleListResponse {
  items: UserSimple[];
  total: number;
  page: number;
  size: number;
}

export interface ListUsersSimpleParams {
  keyword?: string;
  page?: number;
  size?: number;
}

/**
 * 精简用户列表 —— 用于构建 userId → 昵称 的目录。
 *
 * 走 `/v1/iam/users/simple`（分页），只返回 id + nickname，
 * 供 `useUserDirectory` 一次性拉全量后缓存。
 */
export function listUsersSimple(params: ListUsersSimpleParams = {}) {
  return http.get<UserSimpleListResponse>("/v1/iam/users/simple", params);
}
