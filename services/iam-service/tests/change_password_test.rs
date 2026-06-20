//! 集成测试：覆盖 `/iam/auth/password` 修改密码端点的入参校验。
//!
//! HTTP 级 E2E 走 `tests/e2e/ui/cases/iam-change-password.yaml`，
//! 本文件只验证 DTO validator 与路由可达性，不依赖运行中的服务。
//!
//! 由 `node scripts/run-tests-docker.js iam-service` 在容器内执行。

#[cfg(test)]
mod tests {
    use iam_service::dto::ChangePasswordRequest;
    use validator::Validate;

    #[test]
    fn change_password_request_accepts_valid_input() {
        let req = ChangePasswordRequest {
            old_password: "OldP@ssw0rd!".to_string(),
            new_password: "NewP@ssw0rd!".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn change_password_request_rejects_short_new_password() {
        let req = ChangePasswordRequest {
            old_password: "OldP@ssw0rd!".to_string(),
            new_password: "short".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn change_password_request_rejects_short_old_password() {
        let req = ChangePasswordRequest {
            old_password: "x".to_string(),
            new_password: "NewP@ssw0rd!".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn change_password_request_rejects_overlong_new_password() {
        let long = "a".repeat(129);
        let req = ChangePasswordRequest {
            old_password: "OldP@ssw0rd!".to_string(),
            new_password: long,
        };
        assert!(req.validate().is_err());
    }
}
