// 组织架构（iam-service，经网关 /v1/iam/org-units*）。
// 路由事实源：services/iam-service/src/routes/org_units.rs。

import type { HttpCore } from "./http.js";
import type {
  CreateOrgUnitRequest,
  OrgUnitResponse,
  UpdateOrgUnitRequest,
} from "./types.js";

export class OrgUnitsApi {
  constructor(private readonly http: HttpCore) {}

  /** 部门树：GET /v1/iam/org-units（根节点裸数组，children 递归）。 */
  list(): Promise<OrgUnitResponse[]> {
    return this.http.get<OrgUnitResponse[]>("/v1/iam/org-units", { auth: true });
  }

  /** 查询单个部门：GET /v1/iam/org-units/{org_unit_id}。 */
  get(orgUnitId: string): Promise<OrgUnitResponse> {
    return this.http.get<OrgUnitResponse>(
      `/v1/iam/org-units/${encodeURIComponent(orgUnitId)}`,
      { auth: true }
    );
  }

  /** 创建部门：POST /v1/iam/org-units。 */
  create(request: CreateOrgUnitRequest): Promise<OrgUnitResponse> {
    return this.http.post<OrgUnitResponse>("/v1/iam/org-units", request, {
      auth: true,
    });
  }

  /** 更新部门：PUT /v1/iam/org-units/{org_unit_id}。 */
  update(
    orgUnitId: string,
    request: UpdateOrgUnitRequest
  ): Promise<OrgUnitResponse> {
    return this.http.put<OrgUnitResponse>(
      `/v1/iam/org-units/${encodeURIComponent(orgUnitId)}`,
      request,
      { auth: true }
    );
  }

  /** 删除部门：DELETE /v1/iam/org-units/{org_unit_id}，返回 204。 */
  remove(orgUnitId: string): Promise<void> {
    return this.http.delete<void>(
      `/v1/iam/org-units/${encodeURIComponent(orgUnitId)}`,
      { auth: true }
    );
  }

  /** 部门分配用户：PUT /v1/iam/org-units/{org_unit_id}/users，返回 204。 */
  async assignUsers(orgUnitId: string, userIds: string[]): Promise<void> {
    await this.http.put(
      `/v1/iam/org-units/${encodeURIComponent(orgUnitId)}/users`,
      { user_ids: userIds },
      { auth: true }
    );
  }
}
