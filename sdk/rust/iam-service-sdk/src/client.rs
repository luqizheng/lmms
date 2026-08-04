use std::sync::Arc;

use digua_http_client::client::KudiWebClient;
use digua_http_client::{transport::HttpResponse, SdkError};

use crate::model::{
    ChangePasswordRequest, CreateRoleRequest, CreateUserRequest, ListRolesQuery,
    ListUsersQuery, LoginRequest, LoginResponse, PermissionListResponse, RefreshRequest,
    RefreshResponse, RoleListResponse, RoleResponse, UpdateRoleRequest, UpdateUserRequest,
    UserListResponse, UserResponse,
};

type BearerSupplier = Arc<dyn Fn() -> Option<String> + Send + Sync>;

pub struct IamClient {
    base_url: String,
    bearer: BearerSupplier,
    http: KudiWebClient,
}

impl IamClient {
    pub async fn login(
        base_url: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<(Self, LoginResponse), SdkError> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let http = KudiWebClient::builder()
            .build()
            .expect("kudi-http-client: reqwest build");
        let req = LoginRequest {
            username: username.into(),
            password: password.into(),
            tenant_slug: None,
        };
        let url = format!("{}/iam/auth/login", base_url);
        let resp = http
            .post(&url, &serde_json::to_string(&req).unwrap(), None)
            .await?;
        ensure_success(&resp, "iam login")?;
        let login: LoginResponse = serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode LoginResponse: {}", e)))?;
        let access = login.access_token.clone();
        let client = Self {
            base_url,
            bearer: Arc::new(move || Some(access.clone())),
            http,
        };
        Ok((client, login))
    }

    pub fn new<F>(base_url: impl Into<String>, bearer_supplier: F) -> Self
    where
        F: Fn() -> Option<String> + Send + Sync + 'static,
    {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let http = KudiWebClient::builder()
            .build()
            .expect("kudi-http-client: reqwest build");
        Self {
            base_url,
            bearer: Arc::new(bearer_supplier),
            http,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn logout(&self) -> Result<(), SdkError> {
        let url = format!("{}/iam/auth/logout", self.base_url);
        let resp = self.http.post(&url, "{}", (self.bearer)().as_deref()).await?;
        if resp.status == 401 || resp.status == 403 {
            return Ok(());
        }
        ensure_success(&resp, "iam logout")?;
        Ok(())
    }

    pub async fn refresh(&self, refresh_token: &str) -> Result<RefreshResponse, SdkError> {
        let url = format!("{}/iam/auth/refresh", self.base_url);
        let req = RefreshRequest {
            refresh_token: refresh_token.to_string(),
        };
        let body = serde_json::to_string(&req)
            .map_err(|e| SdkError::Network(format!("serialize RefreshRequest: {}", e)))?;
        let resp = self.http.post(&url, &body, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "iam refresh")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode RefreshResponse: {}", e)))
    }

    pub async fn change_password(
        &self,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), SdkError> {
        let url = format!("{}/iam/auth/password", self.base_url);
        let req = ChangePasswordRequest {
            old_password: old_password.to_string(),
            new_password: new_password.to_string(),
        };
        let body = serde_json::to_string(&req)
            .map_err(|e| SdkError::Network(format!("serialize ChangePasswordRequest: {}", e)))?;
        let resp = self.http.post(&url, &body, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "iam change_password")?;
        Ok(())
    }

    pub async fn list_users(&self, q: &ListUsersQuery) -> Result<UserListResponse, SdkError> {
        let url = format!("{}/iam/users?{}", self.base_url, q.to_query_string());
        let resp = self.http.get(&url, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "iam list_users")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode UserListResponse: {}", e)))
    }

    pub async fn get_user(&self, user_id: &str) -> Result<UserResponse, SdkError> {
        let url = format!("{}/iam/users/{}", self.base_url, url_encode_path(user_id));
        let resp = self.http.get(&url, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "iam get_user")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode UserResponse: {}", e)))
    }

    pub async fn create_user(&self, req: &CreateUserRequest) -> Result<UserResponse, SdkError> {
        let url = format!("{}/iam/users", self.base_url);
        let body = serde_json::to_string(req)
            .map_err(|e| SdkError::Network(format!("serialize CreateUserRequest: {}", e)))?;
        let resp = self.http.post(&url, &body, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "iam create_user")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode UserResponse: {}", e)))
    }

    pub async fn update_user(
        &self,
        user_id: &str,
        req: &UpdateUserRequest,
    ) -> Result<UserResponse, SdkError> {
        let url = format!("{}/iam/users/{}", self.base_url, url_encode_path(user_id));
        let body = serde_json::to_string(req)
            .map_err(|e| SdkError::Network(format!("serialize UpdateUserRequest: {}", e)))?;
        let resp = self
            .http
            .post(&url, &body, (self.bearer)().as_deref())
            .await?;
        ensure_success(&resp, "iam update_user")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode UserResponse: {}", e)))
    }

    pub async fn list_roles(&self, q: &ListRolesQuery) -> Result<RoleListResponse, SdkError> {
        let url = format!("{}/iam/roles?{}", self.base_url, q.to_query_string());
        let resp = self.http.get(&url, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "iam list_roles")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode RoleListResponse: {}", e)))
    }

    pub async fn get_role(&self, role_id: &str) -> Result<RoleResponse, SdkError> {
        let url = format!("{}/iam/roles/{}", self.base_url, url_encode_path(role_id));
        let resp = self.http.get(&url, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "iam get_role")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode RoleResponse: {}", e)))
    }

    pub async fn create_role(&self, req: &CreateRoleRequest) -> Result<RoleResponse, SdkError> {
        let url = format!("{}/iam/roles", self.base_url);
        let body = serde_json::to_string(req)
            .map_err(|e| SdkError::Network(format!("serialize CreateRoleRequest: {}", e)))?;
        let resp = self.http.post(&url, &body, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "iam create_role")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode RoleResponse: {}", e)))
    }

    pub async fn update_role(
        &self,
        role_id: &str,
        req: &UpdateRoleRequest,
    ) -> Result<RoleResponse, SdkError> {
        let url = format!("{}/iam/roles/{}", self.base_url, url_encode_path(role_id));
        let body = serde_json::to_string(req)
            .map_err(|e| SdkError::Network(format!("serialize UpdateRoleRequest: {}", e)))?;
        let resp = self
            .http
            .post(&url, &body, (self.bearer)().as_deref())
            .await?;
        ensure_success(&resp, "iam update_role")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode RoleResponse: {}", e)))
    }

    pub async fn list_permissions(&self) -> Result<PermissionListResponse, SdkError> {
        let url = format!("{}/iam/permissions", self.base_url);
        let resp = self.http.get(&url, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "iam list_permissions")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode PermissionListResponse: {}", e)))
    }

    // ── Org-Unit + BatchGetUsers / ListUsersByOrgAndRole methods
    //     were removed (commit feb15f2): those endpoints are service-internal
    //     (require service-jwt, not user bearer through gateway) and the SDK
    //     contract is public user-facing APIs only. See services/iam-service
    //     for internal callers — the OpenAPI public schema does not expose
    //     these routes.

    /// List users by role id with full user details (bearer_auth, not service-jwt).
    pub async fn list_users_by_role(
        &self,
        role_id: &str,
        page: u32,
        size: u32,
    ) -> Result<UserListResponse, SdkError> {
        let url = format!(
            "{}/iam/roles/{}/users?page={}&size={}",
            self.base_url,
            url_encode_path(role_id),
            page,
            size
        );
        let resp = self.http.get(&url, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "iam list_users_by_role")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode UserListResponse: {}", e)))
    }
}

fn ensure_success(resp: &HttpResponse, op: &str) -> Result<(), SdkError> {
    let status = resp.status;
    if (200..300).contains(&status) {
        return Ok(());
    }
    let body = resp.body.clone();
    let msg = format!("{}: HTTP {}", op, status);
    Err(match status {
        401 | 403 => SdkError::Auth { status, message: msg, details: Some(body) },
        429 => SdkError::RateLimit { message: msg, details: Some(body) },
        400 | 422 => SdkError::Validation { status, message: msg, details: Some(body) },
        500..=599 => SdkError::Server { status, message: msg, details: Some(body) },
        _ => SdkError::Network(format!("{}: unexpected status {}", op, status)),
    })
}

fn url_encode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}