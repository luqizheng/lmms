use crate::models::oauth_client::Client;
use crate::service::oauth_client_svc;
use crate::TokenServerState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ApiKeyCreated {
    pub api_key_id: String,
    pub api_key_secret: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateApiKeyRequest {
    pub name: String,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default = "default_grant_types")]
    pub grant_types: Vec<String>,
    pub access_token_ttl_sec: Option<i64>,
    pub refresh_token_ttl_sec: Option<i64>,
    /// 绑定的用户 id；client_credentials 流程用它作为 token 的 sub。
    #[serde(default)]
    pub user_id: Option<String>,
    /// API Key 自身过期时间（RFC3339）；缺省 = 不过期。
    #[serde(default)]
    pub expires_at: Option<String>,
}

fn default_grant_types() -> Vec<String> {
    vec!["client_credentials".to_string()]
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ListApiKeysQuery {
    pub keyword: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ListApiKeysResponse {
    pub items: Vec<ApiKeySummary>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ApiKeySummary {
    pub id: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub status: String,
    pub user_id: Option<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<oauth_client_svc::ClientView> for ApiKeySummary {
    fn from(v: oauth_client_svc::ClientView) -> Self {
        Self {
            id: v.client_id,
            name: v.name,
            permissions: v.permissions,
            status: v.status,
            user_id: v.user_id,
            expires_at: v.expires_at,
            created_at: v.created_at,
        }
    }
}

#[utoipa::path(
    post,
    path = "/api-keys",
    summary = "Create API key (returns plaintext secret once)",
    request_body = CreateApiKeyRequest,
    responses(
        (status = 200, description = "Created api key", body = ApiKeyCreated),
    ),
    tag = "api-key"
)]
pub async fn handle_create_api_key(
    State(state): State<TokenServerState>,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<Json<ApiKeyCreated>, (StatusCode, &'static str)> {
    let expires_at = match req.expires_at.as_deref() {
        None => None,
        Some(raw) => Some(
            chrono::DateTime::parse_from_rfc3339(raw)
                .map_err(|_| (StatusCode::BAD_REQUEST, "invalid expires_at"))?
                .with_timezone(&chrono::Utc),
        ),
    };
    let client_id = uuid::Uuid::new_v4().to_string();
    let client_secret = uuid::Uuid::new_v4().to_string();
    let hash = crate::secret::hash_client_secret(&client_secret);

    let new_client = Client {
        client_id: client_id.clone(),
        client_secret_hash: hash,
        name: req.name,
        permissions: req.permissions,
        grant_types: req.grant_types,
        access_token_ttl_sec: req.access_token_ttl_sec.unwrap_or(3600),
        refresh_token_ttl_sec: req.refresh_token_ttl_sec.unwrap_or(2592000),
        status: "active".into(),
        user_id: req.user_id,
        expires_at,
    };

    oauth_client_svc::insert(&state.db, &new_client)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;

    tracing::info!("created api key client_id={}", client_id);

    Ok(Json(ApiKeyCreated {
        api_key_id: client_id,
        api_key_secret: client_secret,
    }))
}

/// GET /api-keys — 全量列出所有 oauth-clients（当前未区分 api-key / oauth-client，
/// 共享 token_oauth_clients 表）。返回结构去掉 client_secret_hash 防止泄露。
#[utoipa::path(
    get,
    path = "/api-keys",
    params(ListApiKeysQuery),
    responses(
        (status = 200, description = "Paged api-key list", body = ListApiKeysResponse),
    ),
    tag = "api-key"
)]
pub async fn handle_list_api_keys(
    State(state): State<TokenServerState>,
    Query(q): Query<ListApiKeysQuery>,
) -> Result<Json<ListApiKeysResponse>, (StatusCode, &'static str)> {
    let page = q.page.unwrap_or(1);
    let size = q.size.unwrap_or(20);
    let (rows, total) = oauth_client_svc::list(&state.db, q.keyword.as_deref(), None, page, size)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "list failed"))?;

    Ok(Json(ListApiKeysResponse {
        items: rows.into_iter().map(ApiKeySummary::from).collect(),
        total,
        page,
        size,
    }))
}

/// `/api-keys` 路由（带 service-jwt 鉴权，aud=token-service）。
///
/// 创建/列出 API Key 等于发放身份凭据，必须受鉴权保护——原先裸挂，
/// 任何能访问本服务的人都能创建绑定任意用户的密钥（spec §3.5 附带发现）。
pub fn routes(state: crate::TokenServerState) -> axum::Router {
    use axum::routing::{get, post};

    axum::Router::new()
        .route(
            "/api-keys",
            post(handle_create_api_key).get(handle_list_api_keys),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            digua_authz::verify_aud_principal_middleware::<crate::TokenServerState>(
                "token-service",
            ),
        ))
        .with_state(state)
}
