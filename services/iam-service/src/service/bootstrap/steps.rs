//! Bootstrap Step 状态模型（胜出记录 `steps` JSON 的序列化契约）。
//!
//! 设计基线：docs/superpowers/specs/2026-11-28-bootstrap-concurrency-control-design.md
//! Q6/Q7。steps JSON 只保存**非敏感**元数据（key / status / attempts /
//! 安全错误码）——首管理员密码、Bootstrap Password、服务凭据一律不得进入。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// S01：默认租户存在性校验（V003 迁移预置 tenant 1，Check-only）。
pub const STEP_DEFAULT_TENANT: &str = "default_tenant";
/// S02：首管理员用户创建（IAM 本地事务，幂等：存在则核对密码）。
pub const STEP_FIRST_ADMIN_USER: &str = "first_admin_user";
/// S03：首管理员 platform_admin 角色绑定。
pub const STEP_ADMIN_PLATFORM_ROLE: &str = "admin_platform_role";
/// S04：首管理员 tenant_admin 角色绑定。
pub const STEP_ADMIN_TENANT_ROLE: &str = "admin_tenant_role";
/// S05：LMMS 内部服务身份确认（token-service readiness，Check-only）。
pub const STEP_INTERNAL_IDENTITIES: &str = "internal_identities";
/// S06：`iam-service → token-service` Grant（token-service 幂等 provisioning）。
pub const STEP_GRANT_IAM_TO_TOKEN: &str = "grant_iam_to_token";
/// S07：`notification-service → iam-service` Grant（同上）。
pub const STEP_GRANT_NTF_TO_IAM: &str = "grant_ntf_to_iam";

/// 固定步骤清单与执行顺序（LMMS 自主定义；新增内部服务时在此扩展）。
pub const ALL_STEP_KEYS: [&str; 7] = [
    STEP_DEFAULT_TENANT,
    STEP_FIRST_ADMIN_USER,
    STEP_ADMIN_PLATFORM_ROLE,
    STEP_ADMIN_TENANT_ROLE,
    STEP_INTERNAL_IDENTITIES,
    STEP_GRANT_IAM_TO_TOKEN,
    STEP_GRANT_NTF_TO_IAM,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepState {
    pub key: String,
    pub status: StepStatus,
    #[serde(default)]
    pub attempts: u32,
    /// 安全错误摘要（错误码级别，不含凭据 / 拓扑细节）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

/// 胜出记录的步骤状态集合（`iam_bootstrap_init_requests.steps` 列）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepsState {
    pub steps: Vec<StepState>,
}

impl StepsState {
    /// 全 pending 的初始状态（胜出后首次 claim 时写入）。
    pub fn initial() -> Self {
        Self {
            steps: ALL_STEP_KEYS
                .iter()
                .map(|key| StepState {
                    key: (*key).to_string(),
                    status: StepStatus::Pending,
                    attempts: 0,
                    error: None,
                    completed_at: None,
                })
                .collect(),
        }
    }

    /// 从持久化 JSON 解析。缺项（版本演进新增步骤）补 pending；
    /// 解析失败视为损坏 → 从头执行（步骤全部幂等，安全）。
    pub fn parse(value: Option<&Value>) -> Self {
        let mut state = Self::initial();
        let Some(raw) = value else {
            return state;
        };
        let Ok(parsed) = serde_json::from_value::<StepsState>(raw.clone()) else {
            return state;
        };
        for saved in parsed.steps {
            if let Some(slot) = state.steps.iter_mut().find(|s| s.key == saved.key) {
                *slot = saved;
            }
        }
        state
    }

    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    pub fn get(&self, key: &str) -> &StepState {
        self.steps
            .iter()
            .find(|s| s.key == key)
            .unwrap_or_else(|| panic!("step key {key} must exist in ALL_STEP_KEYS"))
    }

    pub fn set_running(&mut self, key: &str) {
        let s = self
            .steps
            .iter_mut()
            .find(|s| s.key == key)
            .expect("step key must exist");
        s.status = StepStatus::Running;
        s.attempts += 1;
        s.error = None;
    }

    pub fn set_completed(&mut self, key: &str) {
        let s = self
            .steps
            .iter_mut()
            .find(|s| s.key == key)
            .expect("step key must exist");
        s.status = StepStatus::Completed;
        s.error = None;
        s.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    pub fn set_failed(&mut self, key: &str, error: &str) {
        let s = self
            .steps
            .iter_mut()
            .find(|s| s.key == key)
            .expect("step key must exist");
        s.status = StepStatus::Failed;
        s.error = Some(error.to_string());
    }

    pub fn all_completed(&self) -> bool {
        self.steps.iter().all(|s| s.status == StepStatus::Completed)
    }

    /// 首管理员创建步骤是否已完成（恢复时决定是否需要重新提交凭据）。
    pub fn first_admin_completed(&self) -> bool {
        self.get(STEP_FIRST_ADMIN_USER).status == StepStatus::Completed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_has_all_steps_pending() {
        let s = StepsState::initial();
        assert_eq!(s.steps.len(), ALL_STEP_KEYS.len());
        assert!(s.steps.iter().all(|s| s.status == StepStatus::Pending));
        assert!(!s.all_completed());
        assert!(!s.first_admin_completed());
    }

    #[test]
    fn parse_roundtrip_and_forward_compat() {
        let mut s = StepsState::initial();
        s.set_running(STEP_DEFAULT_TENANT);
        s.set_completed(STEP_DEFAULT_TENANT);
        let v = s.to_value();
        let parsed = StepsState::parse(Some(&v));
        assert_eq!(parsed.get(STEP_DEFAULT_TENANT).status, StepStatus::Completed);
        assert_eq!(parsed.get(STEP_FIRST_ADMIN_USER).status, StepStatus::Pending);
        // 缺项补 pending：手工裁剪 JSON 后解析仍完整。
        let mut trimmed = v.clone();
        trimmed["steps"] = serde_json::json!([]);
        let empty = StepsState::parse(Some(&trimmed));
        assert_eq!(empty.steps.len(), ALL_STEP_KEYS.len());
        // None / 损坏 JSON → 初始状态。
        assert!(StepsState::parse(None).all_pending());
        assert!(StepsState::parse(Some(&serde_json::json!("garbage"))).all_pending());
    }

    #[test]
    fn steps_json_never_contains_credentials() {
        let mut s = StepsState::initial();
        s.set_running(STEP_FIRST_ADMIN_USER);
        s.set_failed(STEP_FIRST_ADMIN_USER, "IAM_BOOTSTRAP_4004");
        let v = s.to_value().to_string();
        assert!(!v.contains("password"));
        assert!(!v.to_lowercase().contains("secret"));
    }

    impl StepsState {
        fn all_pending(&self) -> bool {
            self.steps.iter().all(|s| s.status == StepStatus::Pending)
        }
    }
}
