// Auto-generated from contracts/error-codes.yaml
// DO NOT EDIT MANUALLY

/// Error code constants
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
        "CFG_VALID_4001" => "config key not found",
        "CFG_VALID_4002" => "config parse error",
        "GW_ROUTE_5001" => "route not found",
        "GW_ROUTE_5002" => "service unavailable",
        "GW_RATE_5003" => "rate limit exceeded",
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
        "CFG_VALID_4001" => ErrorKind::NotFound,
        "CFG_VALID_4002" => ErrorKind::Internal,
        "GW_ROUTE_5001" => ErrorKind::NotFound,
        "GW_ROUTE_5002" => ErrorKind::External,
        "GW_RATE_5003" => ErrorKind::TooManyRequests,
        _ => ErrorKind::Internal,
    }
}