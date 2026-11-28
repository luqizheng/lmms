//! Bootstrap Application Service（WP-01B）：请求准入、2 秒候选仲裁、
//! 执行权与租约、Step 状态持久化、幂等恢复、全步骤完成判定。
//!
//! 业务编排全部在此层（HTTP handler 只做提取与错误映射）。
//!
//! 状态机（设计 Q3/Q5/Q7/Q8）：
//!
//! ```text
//! gate:   idle ──首个准入──▶ collecting ──窗口到期关闭──▶ closed(winner)
//! req:    collecting ──关闭──▶ selected ──claim──▶ running ──▶ completed
//!                                            │            └─步骤失败─▶ failed
//! ```
//!
//! 恢复（Q7/Q8）：服务器重启不自动执行、无后台 worker；用户再次 POST
//! 同一 Bootstrap URL 才会：定位首轮胜出记录（gate.winner_request_id）→
//! 租约允许时原子 claim（epoch+1）→ 读取 steps JSON → 跳过 completed →
//! 继续未完成步骤。rejected 记录永不递补。

pub mod admin_steps;
pub mod executor;
pub mod gate;
pub mod steps;
pub mod token_steps;

use std::time::Duration;

use chrono::Utc;

use crate::dto::{BootstrapRequest, BootstrapResponse, BootstrapStatusResponse, BootstrapStepView};
use crate::error::IamError;
use crate::state::AppState;

use admin_steps::{AdminCredentials, StepFailure};
use executor::ExecutionHandle;
use steps::{StepsState, ALL_STEP_KEYS};

/// 窗口关闭重试：2 秒窗口 + 缓冲，100ms 步进。
const CLOSE_POLL_MAX: u32 = 40;
const CLOSE_POLL_INTERVAL_MS: u64 = 100;
/// 等待窗口结束时的额外缓冲（毫秒），确保关闭事务的 db_now 判定已到期。
const WINDOW_EPSILON_MS: u64 = 30;

/// Bootstrap 密码专用 Header（设计 3.2：只允许 Bootstrap API 使用，不进
/// 日志、不持久化、不进入普通 API 默认 Header）。
pub const INIT_PASSWORD_HEADER: &str = "X-LMMS-Init-Password";

/// 读取 `LMMS_INIT_PASSWORD`（每次请求直读 env：未配置 → Bootstrap 404）。
pub fn init_password() -> Option<String> {
    std::env::var("LMMS_INIT_PASSWORD")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// 常量时间比较（防时序侧信道；不引入新依赖）。
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub struct BootstrapService {
    state: AppState,
    /// 测试注入的密码覆盖（None = 读 env `LMMS_INIT_PASSWORD`）。
    init_password_override: Option<Option<String>>,
    /// 测试注入的 token-service 地址（None = 读 env `TOKEN_SERVICE_URL`）。
    token_service_url_override: Option<String>,
}

impl BootstrapService {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            init_password_override: None,
            token_service_url_override: None,
        }
    }

    /// 测试专用：显式注入 init password（`None` = 模拟未配置）。
    pub fn with_init_password(state: AppState, password: Option<&str>) -> Self {
        Self {
            state,
            init_password_override: Some(password.map(str::to_string)),
            token_service_url_override: None,
        }
    }

    /// 测试专用：注入 token-service 地址（默认读 env `TOKEN_SERVICE_URL`，
    /// 与 main.rs 组装 TokenForwardClient 的来源一致）。
    pub fn with_token_service_url(mut self, url: String) -> Self {
        self.token_service_url_override = Some(url);
        self
    }

    fn effective_password(&self) -> Option<String> {
        match &self.init_password_override {
            Some(pw) => pw.clone(),
            None => init_password(),
        }
    }

    fn effective_token_service_url(&self) -> String {
        self.token_service_url_override.clone().unwrap_or_else(|| {
            std::env::var("TOKEN_SERVICE_URL")
                .unwrap_or_else(|_| "http://token-service:8082".into())
        })
    }

    // ── GET /v1/bootstrap/status ────────────────────────────────────────

    pub async fn status(
        &self,
        password_header: Option<String>,
    ) -> Result<BootstrapStatusResponse, IamError> {
        // 1. 未配置密码 → 404（不查库、不泄漏功能存在性）。
        let expected = self
            .effective_password()
            .ok_or(IamError::BootstrapDisabled)?;

        // 2. 已完成 → 404（接口永久关闭；密码校验之前，设计 Q4/Q9）。
        let gate = gate::read_gate(&self.state.db)
            .await
            .map_err(|e| IamError::Internal(format!("bootstrap gate read failed: {e}")))?;
        let winner = self
            .load_winner(gate.as_ref().and_then(|g| g.winner_request_id))
            .await?;
        if let Some(w) = &winner {
            if w.status == "completed" {
                return Err(IamError::BootstrapCompleted);
            }
        }

        // 3. 密码校验（配置了就必须携带且正确）。
        verify_password(password_header, &expected)?;

        // 4. 组装状态视图。
        let (overall, steps) = match &winner {
            None => ("pending".to_string(), Vec::new()),
            Some(w) => {
                let s = StepsState::parse(w.steps.as_ref());
                let overall = match w.status.as_str() {
                    "failed" => "failed",
                    _ => "initializing",
                };
                (overall.to_string(), step_views(&s))
            }
        };
        Ok(BootstrapStatusResponse {
            status: overall,
            steps,
        })
    }

    // ── POST /v1/bootstrap ──────────────────────────────────────────────

    pub async fn bootstrap(
        &self,
        req: BootstrapRequest,
        password_header: Option<String>,
    ) -> Result<BootstrapResponse, IamError> {
        // 1. 未配置密码 → 404。
        let expected = self
            .effective_password()
            .ok_or(IamError::BootstrapDisabled)?;

        // 2. 前置状态检查：已完成 → 404（不创建请求记录）。
        let gate = gate::read_gate(&self.state.db)
            .await
            .map_err(|e| IamError::Internal(format!("bootstrap gate read failed: {e}")))?;
        let gate = gate.ok_or_else(|| IamError::Internal("bootstrap gate row missing".into()))?;
        let winner = self.load_winner(gate.winner_request_id).await?;
        if let Some(w) = &winner {
            if w.status == "completed" {
                return Err(IamError::BootstrapCompleted);
            }
        }

        // 3. 密码校验（错误 → 403，不创建请求记录）。
        verify_password(password_header, &expected)?;

        // 4. 凭据需求判定：首管理员 Step 未完成（或尚无胜出记录）→ 必须提交。
        let admin_steps_done = winner
            .as_ref()
            .map(|w| StepsState::parse(w.steps.as_ref()).first_admin_completed())
            .unwrap_or(false);
        let creds = if admin_steps::credentials_needed(admin_steps_done) {
            AdminCredentials::from_request(&req)?
        } else {
            None
        };

        // 5. 分派：恢复 / 关闭窗口 / 准入。
        let handle = match winner {
            Some(w) => {
                // gate 已关闭（或至少已有胜出记录）：恢复路径——原子 claim。
                self.claim_or_conflict(w.id).await?
            }
            None => match gate.status.as_str() {
                "closed" => {
                    // 关闭但无胜出记录（空轮防御分支）：无可恢复对象。
                    return Err(IamError::BootstrapConflict(
                        "bootstrap round closed without a winner".into(),
                    ));
                }
                "collecting" => {
                    let now = gate::db_now(&self.state.db).await.map_err(db_err)?;
                    match gate.window_ends_at {
                        Some(ends) if now < ends => {
                            // 首轮窗口进行中：新请求不得加入（设计 Q3 第 8 条）。
                            return Err(IamError::BootstrapConflict(
                                "bootstrap admission window in progress".into(),
                            ));
                        }
                        Some(_) => {
                            // 窗口已到期但尚未关闭（原候选全部崩溃等）：
                            // 本请求作为"任一等待中的请求"触发关闭，随后
                            // 对胜出者走恢复路径。
                            self.close_and_claim().await?
                        }
                        None => {
                            return Err(IamError::Internal(
                                "bootstrap gate window fields missing".into(),
                            ))
                        }
                    }
                }
                _ => {
                    // idle：全新首轮。准入 → 等待窗口 → 关闭 → 判定胜者。
                    self.admit_wait_close_and_claim().await?
                }
            },
        };

        // 6. 执行全部未完成步骤。
        self.run_steps(handle, creds).await
    }

    // ── 内部：胜出记录 / claim ──────────────────────────────────────────

    async fn load_winner(
        &self,
        winner_request_id: Option<i64>,
    ) -> Result<Option<executor::RequestRow>, IamError> {
        executor::find_winner_request(&self.state.db, winner_request_id)
            .await
            .map_err(|e| IamError::Internal(format!("bootstrap request read failed: {e}")))
    }

    async fn claim_or_conflict(&self, request_id: i64) -> Result<ExecutionHandle, IamError> {
        let owner = executor::instance_id();
        executor::claim_execution(&self.state.db, request_id, &owner)
            .await
            .map_err(|e| IamError::Internal(format!("bootstrap claim failed: {e}")))?
            .ok_or_else(|| {
                // 租约未到期（另一实例执行中）或记录不可执行 → 409。
                IamError::BootstrapConflict(
                    "another instance is executing bootstrap (lease active)".into(),
                )
            })
    }

    /// 窗口到期后的关闭 + 对胜出者的 claim（本请求可能是胜者，也可能
    /// 只是触发恢复）。
    async fn close_and_claim(&self) -> Result<ExecutionHandle, IamError> {
        match gate::try_close_window(&self.state.db)
            .await
            .map_err(|e| IamError::Internal(format!("bootstrap close failed: {e}")))?
        {
            gate::CloseOutcome::Closed { winner: Some(id) }
            | gate::CloseOutcome::AlreadyClosed { winner: Some(id) } => {
                self.claim_or_conflict(id).await
            }
            _ => Err(IamError::BootstrapConflict(
                "bootstrap round has no committed candidates".into(),
            )),
        }
    }

    /// 全新首轮：准入 → 等待窗口 → 关闭 → 胜者判定。
    async fn admit_wait_close_and_claim(&self) -> Result<ExecutionHandle, IamError> {
        let my_request_id = match gate::admit_candidate(&self.state.db)
            .await
            .map_err(|e| IamError::Internal(format!("bootstrap admit failed: {e}")))?
        {
            gate::AdmitOutcome::Admitted {
                request_id,
                window_ends_at,
            } => {
                // 2 秒收集窗口：等待（以数据库时钟差值计算，不依赖本地时钟）。
                let now = gate::db_now(&self.state.db).await.map_err(db_err)?;
                let wait = (window_ends_at - now).to_std().unwrap_or(Duration::ZERO);
                tokio::time::sleep(wait + Duration::from_millis(WINDOW_EPSILON_MS)).await;
                request_id
            }
            gate::AdmitOutcome::RejectedClosed | gate::AdmitOutcome::RejectedWindowExpired => {
                return Err(IamError::BootstrapConflict(
                    "bootstrap round already closed".into(),
                ))
            }
        };

        // 关闭窗口（并发下多请求竞争关闭是安全的：gate 行锁串行化）。
        let mut winner: Option<i64> = None;
        for _ in 0..CLOSE_POLL_MAX {
            match gate::try_close_window(&self.state.db)
                .await
                .map_err(|e| IamError::Internal(format!("bootstrap close failed: {e}")))?
            {
                gate::CloseOutcome::Closed { winner: w }
                | gate::CloseOutcome::AlreadyClosed { winner: w } => {
                    winner = w;
                    break;
                }
                gate::CloseOutcome::NotYet { .. } => {
                    tokio::time::sleep(Duration::from_millis(CLOSE_POLL_INTERVAL_MS)).await;
                }
                gate::CloseOutcome::EmptyRound => {
                    // 理论不可达（本请求刚刚提交了候选）；防御性等待重试。
                    tokio::time::sleep(Duration::from_millis(CLOSE_POLL_INTERVAL_MS)).await;
                }
            }
        }
        let Some(winner_id) = winner else {
            return Err(IamError::Internal(
                "bootstrap window close timed out".into(),
            ));
        };

        if winner_id == my_request_id {
            self.claim_or_conflict(winner_id).await
        } else {
            // 非胜出：rejected，不执行初始化（设计 Q3 第 7 条）。
            Err(IamError::BootstrapConflict(
                "another bootstrap request won this round".into(),
            ))
        }
    }

    // ── 内部：步骤执行循环 ──────────────────────────────────────────────

    async fn run_steps(
        &self,
        handle: ExecutionHandle,
        creds: Option<AdminCredentials>,
    ) -> Result<BootstrapResponse, IamError> {
        let row = executor::fetch_request(&self.state.db, handle.request_id)
            .await
            .map_err(|e| IamError::Internal(format!("bootstrap request read failed: {e}")))?
            .ok_or_else(|| IamError::Internal("bootstrap request missing".into()))?;
        let mut steps = StepsState::parse(row.steps.as_ref());

        for key in ALL_STEP_KEYS {
            if steps.get(key).status == steps::StepStatus::Completed {
                continue; // 已完成步骤跳过（幂等恢复）。
            }

            // 每步前续租（owner+epoch 校验；丢失 → 409 让位给新执行者）。
            self.ensure_lease(&handle).await?;

            steps.set_running(key);
            self.persist(&handle, &steps).await?;

            let outcome = self.run_single_step(key, &handle, creds.as_ref()).await;
            match outcome {
                Ok(()) => {
                    steps.set_completed(key);
                    if !self.persist(&handle, &steps).await? {
                        // 执行权被新 epoch 抢走：本实例立即停止（fencing）。
                        return Err(IamError::BootstrapConflict(
                            "bootstrap execution lost to a newer epoch".into(),
                        ));
                    }
                }
                Err(failure) => {
                    let (summary, err) = map_failure(key, &failure);
                    steps.set_failed(key, summary);
                    // mark_failed 同时释放租约（Q7：用户可立即显式恢复）。
                    let _ = executor::mark_step_failed(
                        &self.state.db,
                        &handle,
                        &steps.to_value(),
                        summary,
                        Utc::now(),
                    )
                    .await;
                    return Err(err);
                }
            }
        }

        if !steps.all_completed() {
            return Err(IamError::Internal(
                "bootstrap steps incomplete after execution loop".into(),
            ));
        }
        // 唯一完成入口：全部 Step 完成 → 总状态 completed（此后接口 404）。
        let ok = executor::mark_completed(&self.state.db, &handle, &steps.to_value(), Utc::now())
            .await
            .map_err(|e| IamError::Internal(format!("bootstrap completion failed: {e}")))?;
        if !ok {
            return Err(IamError::BootstrapConflict(
                "bootstrap execution lost to a newer epoch".into(),
            ));
        }

        let admin_username = self
            .admin_username_for_response(creds.as_ref(), handle.request_id)
            .await;
        Ok(BootstrapResponse {
            status: "completed".to_string(),
            admin_username,
            steps: step_views(&steps),
        })
    }

    async fn ensure_lease(&self, handle: &ExecutionHandle) -> Result<(), IamError> {
        if executor::renew_lease(&self.state.db, handle)
            .await
            .map_err(|e| IamError::Internal(format!("bootstrap lease renew failed: {e}")))?
        {
            Ok(())
        } else {
            Err(IamError::BootstrapConflict(
                "bootstrap execution lost to a newer epoch".into(),
            ))
        }
    }

    async fn persist(
        &self,
        handle: &ExecutionHandle,
        steps: &StepsState,
    ) -> Result<bool, IamError> {
        executor::persist_step_progress(&self.state.db, handle, &steps.to_value(), Utc::now())
            .await
            .map_err(|e| IamError::Internal(format!("bootstrap step persist failed: {e}")))
    }

    async fn run_single_step(
        &self,
        key: &str,
        handle: &ExecutionHandle,
        creds: Option<&AdminCredentials>,
    ) -> Result<(), StepFailure> {
        match key {
            steps::STEP_DEFAULT_TENANT => admin_steps::run_default_tenant(&self.state.db).await,
            steps::STEP_FIRST_ADMIN_USER => {
                let Some(creds) = creds else {
                    // S02 未完成但本请求未携带凭据（HTTP 层已校验，防御）。
                    return Err(StepFailure::Failed("admin credentials required"));
                };
                // 事务协议：锁 request 行 → 校验执行权 → Check/Insert/Verify
                // → 同事务原子认领 admin_user_id（R1 / P1-01 + P1-02）。
                admin_steps::run_first_admin_user(&self.state, handle, creds)
                    .await
                    .map(|_| ())
            }
            steps::STEP_ADMIN_PLATFORM_ROLE | steps::STEP_ADMIN_TENANT_ROLE => {
                // 无凭据恢复（Q7）：绑定目标 = 持久化的 admin_user_id，
                // 在绑定事务内 fail-closed 解析，不依赖本次请求凭据。
                if key == steps::STEP_ADMIN_PLATFORM_ROLE {
                    admin_steps::run_admin_platform_role(&self.state, handle).await
                } else {
                    admin_steps::run_admin_tenant_role(&self.state, handle).await
                }
            }
            steps::STEP_INTERNAL_IDENTITIES
            | steps::STEP_GRANT_IAM_TO_TOKEN
            | steps::STEP_GRANT_NTF_TO_IAM => {
                let provisioner = self.provisioner();
                match key {
                    steps::STEP_INTERNAL_IDENTITIES => {
                        token_steps::run_internal_identities(&provisioner).await
                    }
                    steps::STEP_GRANT_IAM_TO_TOKEN => {
                        token_steps::run_grant_iam_to_token(&provisioner).await
                    }
                    _ => token_steps::run_grant_ntf_to_iam(&provisioner).await,
                }
            }
            other => Err(StepFailure::Failed(unknown_step(other))),
        }
    }

    /// 响应中的首管理员用户名（优先本次凭据；无凭据恢复时按认领 id 反查）。
    async fn admin_username_for_response(
        &self,
        creds: Option<&AdminCredentials>,
        request_id: i64,
    ) -> Option<String> {
        if let Some(c) = creds {
            return Some(c.username.clone());
        }
        let user_id = executor::claimed_admin_user_id(&self.state.db, request_id)
            .await
            .ok()??;
        let row: Option<(String,)> = digua_common_db::db_fetch_optional!(
            self.state.db,
            (String,),
            "SELECT username FROM iam_users WHERE id = ? AND deleted_at IS NULL",
            &user_id
        )
        .ok()?;
        row.map(|(u,)| u)
    }

    fn provisioner(&self) -> token_steps::BootstrapProvisioner {
        token_steps::BootstrapProvisioner::new(
            self.state.service_jwt.clone(),
            self.effective_token_service_url(),
        )
    }
}

fn unknown_step(key: &str) -> &'static str {
    tracing::error!(step = %key, "bootstrap unknown step key");
    "unknown step"
}

fn step_status_str(s: &steps::StepStatus) -> &'static str {
    match s {
        steps::StepStatus::Pending => "pending",
        steps::StepStatus::Running => "running",
        steps::StepStatus::Completed => "completed",
        steps::StepStatus::Failed => "failed",
    }
}

fn step_views(steps: &StepsState) -> Vec<BootstrapStepView> {
    steps
        .steps
        .iter()
        .map(|s| BootstrapStepView {
            key: s.key.clone(),
            status: step_status_str(&s.status).to_string(),
            attempts: s.attempts,
        })
        .collect()
}

fn verify_password(header: Option<String>, expected: &str) -> Result<(), IamError> {
    match header {
        Some(provided) if constant_time_eq(&provided, expected) => Ok(()),
        _ => Err(IamError::BootstrapPasswordMismatch),
    }
}

fn map_failure(key: &str, failure: &StepFailure) -> (&'static str, IamError) {
    match failure {
        StepFailure::Conflict(msg) => ("step_conflict", IamError::BootstrapConflict(msg.clone())),
        StepFailure::Failed(code) => (
            code,
            IamError::BootstrapStepFailed(format!("step {key} failed")),
        ),
        StepFailure::Dependency(_) => (
            "dependency_unavailable",
            IamError::BootstrapDependencyUnavailable,
        ),
    }
}

fn db_err(e: digua_common_db::sqlx::Error) -> IamError {
    IamError::Internal(format!("bootstrap db clock read failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_basic() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "abcd"));
        assert!(!constant_time_eq("", "a"));
        assert!(constant_time_eq("", ""));
    }

    #[test]
    fn init_password_missing_when_env_unset() {
        // CI / 测试环境默认未设置（不能依赖进程 env 的现状断言 Some）。
        // 只验证函数不 panic；具体 env 注入断言放在集成测试。
        let _ = init_password();
    }

    #[test]
    fn verify_password_rejects_missing_header() {
        assert!(verify_password(None, "secret").is_err());
        assert!(verify_password(Some("wrong".into()), "secret").is_err());
        assert!(verify_password(Some("secret".into()), "secret").is_ok());
    }
}
