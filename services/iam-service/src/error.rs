use common_error::AppError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum IamError {
    #[error("invalid credentials")]
    InvalidCredentials,
    
    #[error("user not found")]
    UserNotFound,
    
    #[error("user disabled")]
    UserDisabled,
    
    #[error("password expired")]
    PasswordExpired,
    
    #[error("token expired")]
    TokenExpired,
    
    #[error("token revoked")]
    TokenRevoked,
    
    #[error("invalid token")]
    InvalidToken,
    
    #[error("permission denied")]
    PermissionDenied,
    
    #[error("role not found")]
    RoleNotFound,
    
    #[error("invalid role ids: {0:?}")]
    InvalidRoleIds(Vec<String>),
    
    #[error("permission not found")]
    PermissionNotFound,
    
    #[error("menu not found")]
    MenuNotFound,
    
    #[error("org unit not found")]
    OrgUnitNotFound,
    
    #[error("duplicate user")]
    DuplicateUser,
    
    #[error("duplicate role")]
    DuplicateRole,
    
    #[error("duplicate permission")]
    DuplicatePermission,
    
    #[error("duplicate menu")]
    DuplicateMenu,
    
    #[error("duplicate org unit")]
    DuplicateOrgUnit,
    
    #[error("tenant slug already exists")]
    DuplicateTenant,
    
    #[error("conflict: {0}")]
    Conflict(String),
    
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("jwt error: {0}")]
    Jwt(String),
    
    #[error("validation error: {0}")]
    Validation(#[from] validator::ValidationErrors),

    #[error("internal error: {0}")]
    Internal(String),
}

impl From<IamError> for AppError {
    fn from(e: IamError) -> Self {
        match e {
            IamError::InvalidCredentials => AppError::unauthorized("IAM_AUTH_1001", e.to_string()),
            IamError::UserNotFound => AppError::not_found("IAM_USER_2001", e.to_string()),
            IamError::UserDisabled => AppError::forbidden("IAM_USER_2002", e.to_string()),
            IamError::PasswordExpired => AppError::unauthorized("IAM_AUTH_1002", e.to_string()),
            IamError::TokenExpired => AppError::unauthorized("IAM_TOKEN_3001", e.to_string()),
            IamError::TokenRevoked => AppError::unauthorized("IAM_TOKEN_3002", e.to_string()),
            IamError::InvalidToken => AppError::unauthorized("IAM_TOKEN_3003", e.to_string()),
            IamError::PermissionDenied => AppError::forbidden("IAM_AUTHZ_4001", e.to_string()),
            IamError::RoleNotFound => AppError::not_found("IAM_ROLE_5001", e.to_string()),
            IamError::InvalidRoleIds(_) => AppError::bad_request("IAM_ROLE_5003", e.to_string()),
            IamError::PermissionNotFound => AppError::not_found("IAM_PERM_6001", e.to_string()),
            IamError::MenuNotFound => AppError::not_found("IAM_MENU_7001", e.to_string()),
            IamError::OrgUnitNotFound => AppError::not_found("IAM_ORG_8001", e.to_string()),
            IamError::DuplicateUser => AppError::conflict("IAM_USER_2003", e.to_string()),
            IamError::DuplicateRole => AppError::conflict("IAM_ROLE_5002", e.to_string()),
            IamError::DuplicatePermission => AppError::conflict("IAM_PERM_6002", e.to_string()),
            IamError::DuplicateMenu => AppError::conflict("IAM_MENU_7002", e.to_string()),
            IamError::DuplicateOrgUnit => AppError::conflict("IAM_ORG_8002", e.to_string()),
            IamError::DuplicateTenant => AppError::conflict("IAM_TENANT_9100", e.to_string()),
            IamError::Conflict(msg) => AppError::conflict("IAM_SYS_9000", msg),
            IamError::Database(e) => AppError::internal("IAM_SYS_9001", e.to_string()),
            IamError::Jwt(e) => AppError::internal("IAM_SYS_9003", e.to_string()),
            IamError::Validation(e) => AppError::bad_request("IAM_SYS_9005", e.to_string()),
            IamError::Internal(msg) => AppError::internal("IAM_SYS_9006", msg),
        }
    }
}