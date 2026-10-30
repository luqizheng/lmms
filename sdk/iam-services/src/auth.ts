// 认证与账号自服务（iam-service，经网关 /v1/iam/auth/*）。
// 路由事实源：services/iam-service/src/routes/auth.rs。

import type { HttpCore } from "./http.js";
import type {
  ChangePasswordRequest,
  CreateUserRequest,
  LoginRequest,
  LoginResponse,
  SessionInfo,
  SetupStatus,
  UserResponse,
} from "./types.js";

export class AuthApi {
  constructor(private readonly http: HttpCore) {}

  /** 登录：POST /v1/iam/auth/login（公开）。 */
  login(request: LoginRequest): Promise<LoginResponse> {
    return this.http.post<LoginResponse>("/v1/iam/auth/login", request);
  }

  /**
   * 刷新 access token：POST /v1/iam/auth/refresh（公开）。
   * 注意：iam 登录链路的 refresh token 不轮换，响应中的 refresh_token 与传入值相同，
   * 调用方不应因此覆盖已存储的 refresh token。
   */
  refresh(refreshToken: string): Promise<LoginResponse> {
    return this.http.post<LoginResponse>("/v1/iam/auth/refresh", { refresh_token: refreshToken });
  }

  /** 登出：POST /v1/iam/auth/logout。撤销该用户全部 refresh token 并拉黑当前 access token，返回 204。 */
  logout(): Promise<void> {
    return this.http.post<void>("/v1/iam/auth/logout", undefined, { auth: true });
  }

  /** 初始化状态：GET /v1/iam/auth/setup-status（公开）。setup_required=true 时可用 register 创建首个用户。 */
  setupStatus(): Promise<SetupStatus> {
    return this.http.get<SetupStatus>("/v1/iam/auth/setup-status");
  }

  /** 注册首个用户：POST /v1/iam/auth/register（公开；系统已有用户时返回 403，用户名重复返回 409）。 */
  register(request: CreateUserRequest): Promise<UserResponse> {
    return this.http.post<UserResponse>("/v1/iam/auth/register", request);
  }

  /** 修改自己的密码：POST /v1/iam/auth/password。旧密码错误返回 401。 */
  changePassword(request: ChangePasswordRequest): Promise<void> {
    return this.http.post<void>("/v1/iam/auth/password", request, { auth: true });
  }

  /** 当前登录用户的会话列表：GET /v1/iam/auth/sessions。 */
  listSessions(): Promise<SessionInfo[]> {
    return this.http.get<SessionInfo[]>("/v1/iam/auth/sessions", { auth: true });
  }

  /** 撤销一个会话：DELETE /v1/iam/auth/sessions/{session_id}，返回 204。 */
  revokeSession(sessionId: string): Promise<void> {
    return this.http.delete<void>(`/v1/iam/auth/sessions/${encodeURIComponent(sessionId)}`, { auth: true });
  }
}
