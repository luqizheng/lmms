//! `iam-service-sdk` — typed Rust client for the KudiCoder `iam-service`.
//!
//! Endpoints taken from `services/iam-service/openapi.yaml`. Three
//! construction modes:
//!
//! - `IamClient::login(base_url, user, pwd)` — one-shot password login;
//!   the client keeps the returned access token as a bearer supplier.
//! - `IamClient::new(base_url, bearer_supplier)` — caller owns the
//!   bearer (e.g. after refreshing via `client.refresh(...)`).
//! - `IamClient::with_credential(base_url, client_id, secret)` —
//!   `client_credentials` flow (delegates token mgmt to `kudi-http-client`).
//!
//! ```no_run
//! use digua_iam_sdk::IamClient;
//!
//! let _f = async {
//!     let (_client, login) = IamClient::login(
//!         "https://gw.kudi.repos",
//!         "alice",
//!         "secret",
//!     ).await?;
//!     println!("logged in as {} ({})", login.user.username, login.user.id);
//!     Ok::<(), digua_http_client::SdkError>(())
//! };
//! ```

pub mod client;
pub mod model;

pub use crate::client::IamClient;
pub use crate::model::{
    ChangePasswordRequest, CreateRoleRequest, CreateUserRequest, ListRolesQuery, ListUsersQuery,
    LoginRequest, LoginResponse, PermissionListResponse, RefreshRequest, RefreshResponse,
    RoleListResponse, RoleResponse, UpdateRoleRequest, UpdateUserRequest, UserInfo,
    UserListResponse, UserResponse,
};
pub use digua_http_client::SdkError;
