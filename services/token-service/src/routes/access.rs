use crate::models::oauth_client::Client as OAuthClient;
use crate::service::{auth_code_svc, oauth_client_svc};
use crate::TokenServerState;
use argon2::{PasswordHash, PasswordVerifier};
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;

/// 统一的 token 请求体。表单（SDK 的 client_credentials 格式，
/// 见 sdk/rust/digua-http-client/src/fetcher.rs）与 JSON 两种编码都解析到它。
#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    /// authorization_code 流必填；client_credentials 不用。
    #[serde(default)]
    pub code: Option<String>,
    pub client_id: String,
    pub client_secret: String,
}

/// 既有 OpenAPI schema（JSON 编码的授权码交换）。
#[derive(Debug, Deserialize, ToSchema)]
pub struct AccessRequest {
    pub grant_type: String,
    pub code: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AccessResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

#[utoipa::path(
    post,
    path = "/tokens/access",
    summary = "Issue access token via authorization_code or client_credentials",
    request_body = AccessRequest,
    responses(
        (status = 200, description = "issued tokens", body = AccessResponse),
        (status = 400, description = "bad request"),
        (status = 401, description = "invalid client"),
    ),
    tag = "tokens"
)]
pub async fn handle_access(
    State(state): State<TokenServerState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<AccessResponse>, (StatusCode, &'static str)> {
    // SDK 的 client_credentials 用 application/x-www-form-urlencoded（OAuth 惯例），
    // 既有授权码调用方用 JSON —— 按 Content-Type 分发，两者都落到 TokenRequest。
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let parsed: Result<TokenRequest, String> =
        if content_type.starts_with("application/x-www-form-urlencoded") {
            serde_urlencoded::from_bytes(&body).map_err(|e| e.to_string())
        } else {
            serde_json::from_slice(&body).map_err(|e| e.to_string())
        };
    let req: TokenRequest =
        parsed.map_err(|_| (StatusCode::BAD_REQUEST, "invalid request body"))?;

    match req.grant_type.as_str() {
        "client_credentials" => handle_client_credentials(state, req).await,
        "authorization_code" => handle_authorization_code(state, headers, req).await,
        _ => Err((StatusCode::BAD_REQUEST, "unsupported grant_type")),
    }
}

/// 校验 client_secret（argon2，与两条授权流共用）。
fn verify_client_secret(client: &OAuthClient, secret: &str) -> bool {
    PasswordHash::new(&client.client_secret_hash)
        .ok()
        .and_then(|ph| {
            argon2::Argon2::default()
                .verify_password(secret.as_bytes(), &ph)
                .ok()
        })
        .is_some()
}

async fn load_active_client(
    state: &TokenServerState,
    client_id: &str,
    client_secret: &str,
) -> Result<OAuthClient, (StatusCode, &'static str)> {
    let client = oauth_client_svc::find_by_id(&state.db, client_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?
        .ok_or((StatusCode::UNAUTHORIZED, "invalid client"))?;

    if client.status != "active" {
        return Err((StatusCode::FORBIDDEN, "client disabled"));
    }
    if !verify_client_secret(&client, client_secret) {
        return Err((StatusCode::UNAUTHORIZED, "invalid client"));
    }
    Ok(client)
}

/// client_credentials：API Key（绑定真实用户的 client）直接换 token。
///
/// - token 的 `sub` = client 绑定的 user_id（方案 A：AI 工具代表真实用户操作）
/// - `permissions` = 用户 claims ∩ client.permissions（钥匙不得比主人权限更大）
/// - 不签发 refresh_token（RFC 6749 §4.4.3）；续期 = 重新走本流程
async fn handle_client_credentials(
    state: TokenServerState,
    req: TokenRequest,
) -> Result<Json<AccessResponse>, (StatusCode, &'static str)> {
    let client = load_active_client(&state, &req.client_id, &req.client_secret).await?;

    if let Some(exp) = client.expires_at {
        if exp < Utc::now() {
            return Err((StatusCode::UNAUTHORIZED, "client expired"));
        }
    }
    if !client.grant_types.iter().any(|g| g == "client_credentials") {
        return Err((StatusCode::BAD_REQUEST, "grant_type not allowed"));
    }
    let user_id = client
        .user_id
        .clone()
        .ok_or((StatusCode::BAD_REQUEST, "client is not bound to a user"))?;

    // 拉取用户 claims 做权限交集；iam 不可达时 fail-closed（permissions 清空、
    // 不继承降级 claims 里的 roles——机器签发的 token 不得携带兜底 admin 角色）。
    let claims_url = format!(
        "{}/internal/users/{}/claims",
        state.user_service_url, user_id
    );
    let extra_claims: serde_json::Value =
        match crate::fetch_json(&state.http_client, "iam-service", &claims_url, None).await {
            Ok(mut claims) => {
                let user_perms = claims
                    .get("permissions")
                    .and_then(|p| p.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if let Some(obj) = claims.as_object_mut() {
                    obj.insert(
                        "permissions".into(),
                        json!(intersect_permissions(&user_perms, &client.permissions)),
                    );
                    obj.entry("user_code".to_string())
                        .or_insert_with(|| json!(user_id));
                }
                claims
            }
            Err(e) => {
                tracing::warn!(
                    "user-service unreachable at {}: {}, issuing client_credentials token with empty permissions",
                    claims_url,
                    e
                );
                json!({"permissions": [], "user_code": user_id})
            }
        };

    let issuer = &state.issuer;
    let at = issuer
        .issue_access_token(
            &user_id,
            &req.client_id,
            Some(crate::issuer::purpose::API_KEY),
            extra_claims,
            client.access_token_ttl_sec,
        )
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "token issue failed"))?;

    Ok(Json(AccessResponse {
        access_token: at,
        // RFC 6749 §4.4.3: client_credentials SHOULD NOT include a refresh token
        refresh_token: String::new(),
        token_type: "Bearer".into(),
        expires_in: client.access_token_ttl_sec,
    }))
}

/// 用户权限 ∩ client 权限：API Key 不得比钥匙主人权限更大。
fn intersect_permissions(user: &[String], client: &[String]) -> Vec<String> {
    user.iter()
        .filter(|p| client.contains(p))
        .cloned()
        .collect()
}

async fn handle_authorization_code(
    state: TokenServerState,
    headers: HeaderMap,
    req: TokenRequest,
) -> Result<Json<AccessResponse>, (StatusCode, &'static str)> {
    let code = req.code.unwrap_or_default();
    let client = load_active_client(&state, &req.client_id, &req.client_secret).await?;

    let user_id = auth_code_svc::consume(&state.db, &code)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?
        .ok_or((StatusCode::BAD_REQUEST, "invalid or expired code"))?;

    // TODO(8.3-fu1): auth code flow lacks tenant context — get_claims requires ?tenant_id=.
    // The auth code table doesn't carry tenant_id, and we can't know it before calling
    // get_claims (chicken-and-egg). The call will 400 and fall through to default claims
    // below. Proper fix: propagate tenant_id through auth_code_svc or a dedicated lookup.
    let claims_url = format!(
        "{}/internal/users/{}/claims",
        state.user_service_url, user_id
    );
    let user_jwt = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let extra_claims: serde_json::Value =
        match crate::fetch_json(&state.http_client, "iam-service", &claims_url, user_jwt).await {
            Ok(claims) => claims,
            Err(e) => {
                tracing::warn!(
                    "user-service unreachable at {}: {}, using default claims",
                    claims_url,
                    e
                );
                json!({
                    "roles": ["admin"],
                    "permissions": ["notification.admin"]
                })
            }
        };

    let issuer = &state.issuer;
    let at = issuer
        .issue_access_token(
            &user_id,
            &req.client_id,
            Some(crate::issuer::purpose::AUTH_CODE),
            extra_claims,
            client.access_token_ttl_sec,
        )
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "token issue failed"))?;

    let (rt, _family) = issuer
        .issue_refresh_token(
            &user_id,
            Some(crate::issuer::purpose::AUTH_CODE),
            client.refresh_token_ttl_sec,
        )
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "token issue failed"))?;

    Ok(Json(AccessResponse {
        access_token: at,
        refresh_token: rt,
        token_type: "Bearer".into(),
        expires_in: client.access_token_ttl_sec,
    }))
}
