//! Bootstrap 跨服务步骤（S05~S07）：IAM 对 Token Service 的内部初始化调用。
//!
//! 设计基线 Q12：内部服务身份与授权关系**完全由 LMMS 定义**——固定清单
//! 以代码形式落在 token-service 的 `SYSTEM_GRANTS`（单一真相源），AI-PM
//! 请求不得传入。IAM 通过 token-service 的内部 provisioning 端点做
//! Check / 幂等 ensure：
//!
//! - `GET  /internal/bootstrap/readiness`  → 必需身份 + 必需 Grant 的状态
//!   （无任何凭据材料）。
//! - `POST /internal/bootstrap/ensure-grants` → 幂等补插缺失的系统 Grant
//!   （system_managed = TRUE）；已存在但被禁用的行 → conflicts（不覆盖）。
//!
//! 鉴权复用现有服务间机制：service-jwt（`aud=token-service`），由
//! `ServiceJwtCache` 从 IAM 自己的 bootstrap token 换取——Bootstrap
//! Password **绝不**用于服务间认证。
//!
//! 幂等键 = (owner, audience)（grant 表主键）/ owner（身份表主键）；
//! 重试与恢复不会产生重复身份或重复 Grant。

use std::sync::Arc;
use std::time::Duration;

use digua_common_jwt::ServiceJwtCache;
use serde::Deserialize;

use super::admin_steps::StepFailure;

/// LMMS 内部必需服务身份（与 token-service SYSTEM_GRANTS 的 owner 并集
/// 一致；token-service 对 iam-service 主体缺失时拒绝启动）。
pub const REQUIRED_IDENTITIES: [&str; 2] = ["iam-service", "notification-service"];
/// S06 对应的既有内部授权关系：iam-service → token-service（登录链）。
pub const GRANT_IAM_TO_TOKEN: (&str, &str) = ("iam-service", "token-service");
/// S07 对应的既有内部授权关系：notification-service → iam-service。
pub const GRANT_NTF_TO_IAM: (&str, &str) = ("notification-service", "iam-service");

/// 身份状态（无凭据材料）。
#[derive(Debug, Clone, Deserialize)]
pub struct IdentityState {
    pub owner: String,
    pub exists: bool,
    #[serde(default)]
    pub status: Option<String>,
}

/// Grant 状态（无凭据材料）。
#[derive(Debug, Clone, Deserialize)]
pub struct GrantState {
    pub owner: String,
    pub audience: String,
    pub exists: bool,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub system_managed: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReadinessResponse {
    pub identities: Vec<IdentityState>,
    pub grants: Vec<GrantState>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnsureGrantsResponse {
    #[serde(default)]
    pub ensured: Vec<GrantState>,
    #[serde(default)]
    pub conflicts: Vec<GrantState>,
}

/// token-service provisioning 调用错误。
#[derive(Debug)]
pub enum ProvisionError {
    /// 网络 / 超时 / 5xx——依赖暂不可用，可恢复。
    Unreachable(String),
    /// 服务端明确拒绝（鉴权等）。
    Rejected(u16),
    /// 响应不符合契约。
    Malformed(String),
}

/// IAM → token-service 的 Bootstrap provisioning 客户端。
pub struct BootstrapProvisioner {
    service_jwt: Arc<ServiceJwtCache>,
    token_service_url: String,
    http: reqwest::Client,
}

impl BootstrapProvisioner {
    pub fn new(service_jwt: Arc<ServiceJwtCache>, token_service_url: String) -> Self {
        Self {
            service_jwt,
            token_service_url,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("reqwest client"),
        }
    }

    fn map_err(e: reqwest::Error) -> ProvisionError {
        if e.is_timeout() || e.is_connect() || e.is_request() {
            ProvisionError::Unreachable(e.to_string())
        } else {
            ProvisionError::Unreachable(format!("token-service request failed: {e}"))
        }
    }

    async fn get(&self, path: &str) -> Result<reqwest::Response, ProvisionError> {
        let service_jwt = self
            .service_jwt
            .get("token-service")
            .await
            .map_err(|e| ProvisionError::Unreachable(format!("service-jwt mint failed: {e}")))?;
        let url = format!("{}/internal/bootstrap{path}", self.token_service_url.trim_end_matches('/'));
        self.http
            .get(url)
            .bearer_auth(&service_jwt)
            .send()
            .await
            .map_err(Self::map_err)
    }

    async fn post(&self, path: &str) -> Result<reqwest::Response, ProvisionError> {
        let service_jwt = self
            .service_jwt
            .get("token-service")
            .await
            .map_err(|e| ProvisionError::Unreachable(format!("service-jwt mint failed: {e}")))?;
        let url = format!("{}/internal/bootstrap{path}", self.token_service_url.trim_end_matches('/'));
        self.http
            .post(url)
            .bearer_auth(&service_jwt)
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(Self::map_err)
    }

    async fn parse<T: for<'de> Deserialize<'de>>(
        resp: reqwest::Response,
    ) -> Result<T, ProvisionError> {
        let status = resp.status();
        if !status.is_success() {
            return Err(if status.is_server_error() {
                ProvisionError::Unreachable(format!("token-service {status}"))
            } else {
                ProvisionError::Rejected(status.as_u16())
            });
        }
        resp.json::<T>()
            .await
            .map_err(|e| ProvisionError::Malformed(e.to_string()))
    }

    /// S05 数据源：必需身份 + 必需 Grant 状态（token-service 按自己的
    /// SYSTEM_GRANTS 清单计算，IAM 不传任何拓扑）。
    pub async fn readiness(&self) -> Result<ReadinessResponse, ProvisionError> {
        let resp = self.get("/readiness").await?;
        Self::parse(resp).await
    }

    /// S06/S07 数据源：幂等 ensure 全部系统 Grant（稳定幂等键 owner+audience）。
    pub async fn ensure_grants(&self) -> Result<EnsureGrantsResponse, ProvisionError> {
        let resp = self.post("/ensure-grants").await?;
        Self::parse(resp).await
    }
}

// ── S05：内部服务身份确认（Check-only）──────────────────────────────────

pub async fn run_internal_identities(
    provisioner: &BootstrapProvisioner,
) -> Result<(), StepFailure> {
    let readiness = match provisioner.readiness().await {
        Ok(r) => r,
        Err(ProvisionError::Unreachable(_)) | Err(ProvisionError::Rejected(_)) => {
            return Err(StepFailure::Dependency("token service unavailable"))
        }
        Err(ProvisionError::Malformed(_)) => {
            return Err(StepFailure::Dependency("token service response invalid"))
        }
    };
    for owner in REQUIRED_IDENTITIES {
        let ok = readiness.identities.iter().any(|i| {
            i.owner == owner && i.exists && i.status.as_deref() == Some("active")
        });
        if !ok {
            // 摘要不含服务名（不向调用方泄漏内部拓扑细节）。
            return Err(StepFailure::Failed("required service identity not ready"));
        }
    }
    Ok(())
}

// ── S06 / S07：内部授权关系 ensure（幂等 provisioning）──────────────────

pub async fn run_grant_iam_to_token(
    provisioner: &BootstrapProvisioner,
) -> Result<(), StepFailure> {
    ensure_single_grant(provisioner, GRANT_IAM_TO_TOKEN.0, GRANT_IAM_TO_TOKEN.1)
        .await
}

pub async fn run_grant_ntf_to_iam(
    provisioner: &BootstrapProvisioner,
) -> Result<(), StepFailure> {
    ensure_single_grant(provisioner, GRANT_NTF_TO_IAM.0, GRANT_NTF_TO_IAM.1).await
}

async fn ensure_single_grant(
    provisioner: &BootstrapProvisioner,
    owner: &str,
    audience: &str,
) -> Result<(), StepFailure> {
    let resp = match provisioner.ensure_grants().await {
        Ok(r) => r,
        Err(ProvisionError::Unreachable(_)) | Err(ProvisionError::Rejected(_)) => {
            return Err(StepFailure::Dependency("token service unavailable"))
        }
        Err(ProvisionError::Malformed(_)) => {
            return Err(StepFailure::Dependency("token service response invalid"))
        }
    };
    // 已存在但被禁用 → 冲突（不覆盖既有授权）。
    if resp
        .conflicts
        .iter()
        .any(|c| c.owner == owner && c.audience == audience)
    {
        return Err(StepFailure::Conflict(
            "required internal grant exists but is disabled".into(),
        ));
    }
    let entry = resp
        .ensured
        .iter()
        .find(|g| g.owner == owner && g.audience == audience);
    match entry {
        Some(g) if g.exists && g.status.as_deref() == Some("active") => Ok(()),
        Some(_) => Err(StepFailure::Failed("required internal grant not active")),
        None => Err(StepFailure::Failed("required internal grant missing")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_identities_cover_known_topology() {
        assert_eq!(REQUIRED_IDENTITIES.len(), 2);
        assert!(REQUIRED_IDENTITIES.contains(&"iam-service"));
        assert!(REQUIRED_IDENTITIES.contains(&"notification-service"));
    }

    #[test]
    fn grant_pairs_match_design() {
        assert_eq!(GRANT_IAM_TO_TOKEN, ("iam-service", "token-service"));
        assert_eq!(GRANT_NTF_TO_IAM, ("notification-service", "iam-service"));
    }

    #[test]
    fn readiness_response_parses_minimal_json() {
        let raw = serde_json::json!({
            "identities": [{"owner": "iam-service", "exists": true, "status": "active"}],
            "grants": [{"owner": "iam-service", "audience": "token-service", "exists": true}]
        });
        let r: ReadinessResponse = serde_json::from_value(raw).expect("parse");
        assert_eq!(r.identities.len(), 1);
        assert_eq!(r.grants[0].system_managed, None);
    }

    #[test]
    fn ensure_response_defaults_empty() {
        let raw = serde_json::json!({});
        let r: EnsureGrantsResponse = serde_json::from_value(raw).expect("parse");
        assert!(r.ensured.is_empty());
        assert!(r.conflicts.is_empty());
    }
}
