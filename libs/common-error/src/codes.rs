// Auto-generated from contracts/error-codes.yaml
// DO NOT EDIT MANUALLY

/// Error code constants
#[allow(clippy::module_inception)]
pub mod codes {
    /// internal server error
    pub const SYS_INTERNAL_0001: &str = "SYS_INTERNAL_0001";
    /// database error
    pub const SYS_DB_0001: &str = "SYS_DB_0001";
    /// row not found
    pub const SYS_DB_0002: &str = "SYS_DB_0002";
    /// http client error
    pub const SYS_HTTP_0001: &str = "SYS_HTTP_0001";
    /// invalid credentials
    pub const IAM_AUTH_1001: &str = "IAM_AUTH_1001";
    /// token expired
    pub const IAM_AUTH_1002: &str = "IAM_AUTH_1002";
    /// token revoked
    pub const IAM_AUTH_1003: &str = "IAM_AUTH_1003";
    /// invalid token
    pub const IAM_AUTH_1004: &str = "IAM_AUTH_1004";
    /// missing token
    pub const IAM_AUTH_1005: &str = "IAM_AUTH_1005";
    /// password too weak
    pub const IAM_AUTH_1006: &str = "IAM_AUTH_1006";
    /// username already exists
    pub const IAM_AUTH_1007: &str = "IAM_AUTH_1007";
    /// email already exists
    pub const IAM_AUTH_1008: &str = "IAM_AUTH_1008";
    /// invalid input
    pub const VALID_2001: &str = "VALID_2001";
    /// missing required field
    pub const VALID_2002: &str = "VALID_2002";
    /// invalid phone format
    pub const VALID_2003: &str = "VALID_2003";
    /// invalid email format
    pub const VALID_2004: &str = "VALID_2004";
    /// permission denied
    pub const IAM_PERM_3001: &str = "IAM_PERM_3001";
    /// role not found
    pub const IAM_PERM_3002: &str = "IAM_PERM_3002";
    /// config item not found
    pub const CFG_ITEM_1001: &str = "CFG_ITEM_1001";
    /// namespace not found
    pub const CFG_NS_2001: &str = "CFG_NS_2001";
    /// validation failed
    pub const CFG_VALID_3001: &str = "CFG_VALID_3001";
    /// config key not found
    pub const CFG_VALID_4001: &str = "CFG_VALID_4001";
    /// config parse error
    pub const CFG_VALID_4002: &str = "CFG_VALID_4002";
    /// route not found
    pub const GW_ROUTE_5001: &str = "GW_ROUTE_5001";
    /// service unavailable
    pub const GW_ROUTE_5002: &str = "GW_ROUTE_5002";
    /// rate limit exceeded
    pub const GW_RATE_5003: &str = "GW_RATE_5003";
    /// audit log not found
    pub const AUD_LOG_6001: &str = "AUD_LOG_6001";
    /// export format not supported
    pub const AUD_LOG_6002: &str = "AUD_LOG_6002";
    /// export generation failed
    pub const AUD_LOG_6003: &str = "AUD_LOG_6003";
    /// file not found
    pub const FILE_ITEM_1001: &str = "FILE_ITEM_1001";
    /// file upload failed
    pub const FILE_UPLOAD_2001: &str = "FILE_UPLOAD_2001";
    /// file too large
    pub const FILE_UPLOAD_2002: &str = "FILE_UPLOAD_2002";
    /// presign url expired
    pub const FILE_PRESIGN_3001: &str = "FILE_PRESIGN_3001";
    /// file read failed
    pub const FILE_READ_4001: &str = "FILE_READ_4001";
    /// refine forbidden for caller
    pub const FILE_DELETE_4002: &str = "FILE_DELETE_4002";
}

/// Get message for error code
pub fn message(code: &str) -> &str {
    match code {
        "SYS_INTERNAL_0001" => "internal server error",
        "SYS_DB_0001" => "database error",
        "SYS_DB_0002" => "row not found",
        "SYS_HTTP_0001" => "http client error",
        "IAM_AUTH_1001" => "invalid credentials",
        "IAM_AUTH_1002" => "token expired",
        "IAM_AUTH_1003" => "token revoked",
        "IAM_AUTH_1004" => "invalid token",
        "IAM_AUTH_1005" => "missing token",
        "IAM_AUTH_1006" => "password too weak",
        "IAM_AUTH_1007" => "username already exists",
        "IAM_AUTH_1008" => "email already exists",
        "VALID_2001" => "invalid input",
        "VALID_2002" => "missing required field",
        "VALID_2003" => "invalid phone format",
        "VALID_2004" => "invalid email format",
        "IAM_PERM_3001" => "permission denied",
        "IAM_PERM_3002" => "role not found",
        "CFG_ITEM_1001" => "config item not found",
        "CFG_NS_2001" => "namespace not found",
        "CFG_VALID_3001" => "validation failed",
        "CFG_VALID_4001" => "config key not found",
        "CFG_VALID_4002" => "config parse error",
        "GW_ROUTE_5001" => "route not found",
        "GW_ROUTE_5002" => "service unavailable",
        "GW_RATE_5003" => "rate limit exceeded",
        "AUD_LOG_6001" => "audit log not found",
        "AUD_LOG_6002" => "export format not supported",
        "AUD_LOG_6003" => "export generation failed",
        "FILE_ITEM_1001" => "file not found",
        "FILE_UPLOAD_2001" => "file upload failed",
        "FILE_UPLOAD_2002" => "file too large",
        "FILE_PRESIGN_3001" => "presign url expired",
        "FILE_READ_4001" => "file read failed",
        "FILE_DELETE_4002" => "refine forbidden for caller",
        _ => "unknown error",
    }
}

use crate::ErrorKind;

/// Get ErrorKind for error code
pub fn kind(code: &str) -> ErrorKind {
    match code {
        "SYS_INTERNAL_0001" => ErrorKind::Internal,
        "SYS_DB_0001" => ErrorKind::Internal,
        "SYS_DB_0002" => ErrorKind::NotFound,
        "SYS_HTTP_0001" => ErrorKind::External,
        "IAM_AUTH_1001" => ErrorKind::Unauthorized,
        "IAM_AUTH_1002" => ErrorKind::Unauthorized,
        "IAM_AUTH_1003" => ErrorKind::Unauthorized,
        "IAM_AUTH_1004" => ErrorKind::Unauthorized,
        "IAM_AUTH_1005" => ErrorKind::Unauthorized,
        "IAM_AUTH_1006" => ErrorKind::BadRequest,
        "IAM_AUTH_1007" => ErrorKind::Conflict,
        "IAM_AUTH_1008" => ErrorKind::Conflict,
        "VALID_2001" => ErrorKind::BadRequest,
        "VALID_2002" => ErrorKind::BadRequest,
        "VALID_2003" => ErrorKind::BadRequest,
        "VALID_2004" => ErrorKind::BadRequest,
        "IAM_PERM_3001" => ErrorKind::Forbidden,
        "IAM_PERM_3002" => ErrorKind::NotFound,
        "CFG_ITEM_1001" => ErrorKind::NotFound,
        "CFG_NS_2001" => ErrorKind::NotFound,
        "CFG_VALID_3001" => ErrorKind::BadRequest,
        "CFG_VALID_4001" => ErrorKind::NotFound,
        "CFG_VALID_4002" => ErrorKind::Internal,
        "GW_ROUTE_5001" => ErrorKind::NotFound,
        "GW_ROUTE_5002" => ErrorKind::External,
        "GW_RATE_5003" => ErrorKind::TooManyRequests,
        "AUD_LOG_6001" => ErrorKind::NotFound,
        "AUD_LOG_6002" => ErrorKind::BadRequest,
        "AUD_LOG_6003" => ErrorKind::Internal,
        "FILE_ITEM_1001" => ErrorKind::NotFound,
        "FILE_UPLOAD_2001" => ErrorKind::Internal,
        "FILE_UPLOAD_2002" => ErrorKind::BadRequest,
        "FILE_PRESIGN_3001" => ErrorKind::Unauthorized,
        "FILE_READ_4001" => ErrorKind::Internal,
        "FILE_DELETE_4002" => ErrorKind::Forbidden,
        _ => ErrorKind::Internal,
    }
}
