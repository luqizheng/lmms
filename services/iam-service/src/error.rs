use common_error::{AppError, ErrorCode};
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
    
    #[error("conflict: {0}")]
    Conflict(String),
    
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
    
    #[error("jwt error: {0}")]
    Jwt(String),
    
    #[error("crypto error: {0}")]
    Crypto(#[from] common_crypto::CryptoError),
    
    #[error("validation error: {0}")]
    Validation(#[from] validator::ValidationErrors),
}

impl From<IamError> for AppError {
    fn from(e: IamError) -> Self {
        match e {
            IamError::InvalidCredentials => AppError::unauthorized(ErrorCode::IAM_AUTH_1001, e.to_string()),
            IamError::UserNotFound => AppError::not_found(ErrorCode::IAM_USER_2001, e.to_string()),
            IamError::UserDisabled => AppError::forbidden(ErrorCode::IAM_USER_2002, e.to_string()),
            IamError::PasswordExpired => AppError::unauthorized(ErrorCode::IAM_AUTH_1002, e.to_string()),
            IamError::TokenExpired => AppError::unauthorized(ErrorCode::IAM_TOKEN_3001, e.to_string()),
            IamError::TokenRevoked => AppError::unauthorized(ErrorCode::IAM_TOKEN_3002, e.to_string()),
            IamError::InvalidToken => AppError::unauthorized(ErrorCode::IAM_TOKEN_3003, e.to_string()),
            IamError::PermissionDenied => AppError::forbidden(ErrorCode::IAM_AUTHZ_4001, e.to_string()),
            IamError::RoleNotFound => AppError::not_found(ErrorCode::IAM_ROLE_5001, e.to_string()),
            IamError::PermissionNotFound => AppError::not_found(ErrorCode::IAM_PERM_6001, e.to_string()),
            IamError::MenuNotFound => AppError::not_found(ErrorCode::IAM_MENU_7001, e.to_string()),
            IamError::OrgUnitNotFound => AppError::not_found(ErrorCode::IAM_ORG_8001, e.to_string()),
            IamError::DuplicateUser => AppError::conflict(ErrorCode::IAM_USER_2003, e.to_string()),
            IamError::DuplicateRole => AppError::conflict(ErrorCode::IAM_ROLE_5002, e.to_string()),
            IamError::DuplicatePermission => AppError::conflict(ErrorCode::IAM_PERM_6002, e.to_string()),
            IamError::DuplicateMenu => AppError::conflict(ErrorCode::IAM_MENU_7002, e.to_string()),
            IamError::DuplicateOrgUnit => AppError::conflict(ErrorCode::IAM_ORG_8002, e.to_string()),
            IamError::Conflict(msg) => AppError::conflict(ErrorCode::IAM_SYS_9000, msg),
            IamError::Database(e) => AppError::internal(ErrorCode::IAM_SYS_9001, e.to_string()),
            IamError::Redis(e) => AppError::internal(ErrorCode::IAM_SYS_9002, e.to_string()),
            IamError::Jwt(e) => AppError::internal(ErrorCode::IAM_SYS_9003, e.to_string()),
            IamError::Crypto(e) => AppError::internal(ErrorCode::IAM_SYS_9004, e.to_string()),
            IamError::Validation(e) => AppError::bad_request(ErrorCode::IAM_SYS_9005, e.to_string()),
        }
    }
}
