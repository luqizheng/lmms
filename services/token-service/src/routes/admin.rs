use crate::secret::hash_client_secret;
use crate::service::oauth_client_svc::{
    self, generate_client_id, generate_client_secret, ClientPatch, ClientView,
};
use crate::TokenServerState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListQuery {
    pub keyword: Option<String>,
    pub status: Option<String>,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_size")]
    pub size: u32,
}

fn default_page() -> u32 {
    1
}
fn default_size() -> u32 {
    10
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ClientDto {
    pub client_id: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub grant_types: Vec<String>,
    pub access_token_ttl_sec: i64,
    pub refresh_token_ttl_sec: i64,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<ClientView> for ClientDto {
    fn from(v: ClientView) -> Self {
        Self {
            client_id: v.client_id,
            name: v.name,
            permissions: v.permissions,
            grant_types: v.grant_types,
            access_token_ttl_sec: v.access_token_ttl_sec,
            refresh_token_ttl_sec: v.refresh_token_ttl_sec,
            status: v.status,
            created_at: v.created_at,
            updated_at: v.updated_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListResponse {
    pub list: Vec<ClientDto>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRequest {
    pub name: String,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub grant_types: Vec<String>,
    #[serde(default = "default_access_ttl")]
    pub access_token_ttl_sec: i64,
    #[serde(default = "default_refresh_ttl")]
    pub refresh_token_ttl_sec: i64,
    #[serde(default = "default_status")]
    pub status: String,
}

fn default_access_ttl() -> i64 {
    3600
}
fn default_refresh_ttl() -> i64 {
    2592000
}
fn default_status() -> String {
    "active".to_string()
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateResponse {
    pub client_id: String,
    pub client_secret: String,
    pub client: ClientDto,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateRequest {
    pub name: Option<String>,
    pub permissions: Option<Vec<String>>,
    pub grant_types: Option<Vec<String>>,
    pub access_token_ttl_sec: Option<i64>,
    pub refresh_token_ttl_sec: Option<i64>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ResetSecretResponse {
    pub client_id: String,
    pub client_secret: String,
}

fn internal(_e: sqlx::Error) -> (StatusCode, &'static str) {
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/oauth-clients",
    summary = "List OAuth clients",
    params(ListQuery),
    responses((status = 200, body = ListResponse)),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_list(
    State(state): State<TokenServerState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse>, (StatusCode, &'static str)> {
    let (rows, total) = oauth_client_svc::list(
        &state.db,
        q.keyword.as_deref().filter(|s| !s.is_empty()),
        q.status.as_deref().filter(|s| !s.is_empty()),
        q.page,
        q.size,
    )
    .await
    .map_err(internal)?;
    Ok(Json(ListResponse {
        list: rows.into_iter().map(ClientDto::from).collect(),
        total,
        page: q.page.max(1),
        size: q.size.clamp(1, 200),
    }))
}

#[utoipa::path(
    post,
    path = "/admin/oauth-clients",
    summary = "Create OAuth client (returns plaintext client_secret once)",
    request_body = CreateRequest,
    responses(
        (status = 200, body = CreateResponse),
        (status = 400, description = "bad request"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_create(
    State(state): State<TokenServerState>,
    Json(req): Json<CreateRequest>,
) -> Result<Json<CreateResponse>, (StatusCode, &'static str)> {
    if req.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name required"));
    }
    let plain_secret = generate_client_secret();
    let hash = hash_client_secret(&plain_secret);
    let client_id = generate_client_id(&req.name);
    let model = crate::models::oauth_client::Client {
        client_id: client_id.clone(),
        client_secret_hash: hash,
        name: req.name,
        permissions: req.permissions,
        grant_types: req.grant_types,
        access_token_ttl_sec: req.access_token_ttl_sec,
        refresh_token_ttl_sec: req.refresh_token_ttl_sec,
        status: req.status,
    };
    oauth_client_svc::insert(&state.db, &model)
        .await
        .map_err(internal)?;

    let view = oauth_client_svc::list(&state.db, Some(&client_id), None, 1, 1)
        .await
        .map_err(internal)?
        .0
        .into_iter()
        .next()
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "client missing after insert",
        ))?;

    Ok(Json(CreateResponse {
        client_id,
        client_secret: plain_secret,
        client: ClientDto::from(view),
    }))
}

#[utoipa::path(
    put,
    path = "/admin/oauth-clients/{client_id}",
    summary = "Update OAuth client",
    params(("client_id" = String, Path,)),
    request_body = UpdateRequest,
    responses(
        (status = 200, body = ClientDto),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_update(
    State(state): State<TokenServerState>,
    Path(client_id): Path<String>,
    Json(req): Json<UpdateRequest>,
) -> Result<Json<ClientDto>, (StatusCode, &'static str)> {
    let patch = ClientPatch {
        name: req.name,
        permissions: req.permissions,
        grant_types: req.grant_types,
        access_token_ttl_sec: req.access_token_ttl_sec,
        refresh_token_ttl_sec: req.refresh_token_ttl_sec,
        status: req.status,
    };
    let updated = oauth_client_svc::update(&state.db, &client_id, &patch)
        .await
        .map_err(internal)?;
    if !updated {
        return Err((StatusCode::NOT_FOUND, "client not found"));
    }
    let view = oauth_client_svc::list(&state.db, Some(&client_id), None, 1, 1)
        .await
        .map_err(internal)?
        .0
        .into_iter()
        .next()
        .ok_or((StatusCode::NOT_FOUND, "client not found"))?;
    Ok(Json(ClientDto::from(view)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/oauth-clients/{client_id}",
    summary = "Delete OAuth client",
    params(("client_id" = String, Path,)),
    responses(
        (status = 204, description = "deleted"),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_delete(
    State(state): State<TokenServerState>,
    Path(client_id): Path<String>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let ok = oauth_client_svc::delete(&state.db, &client_id)
        .await
        .map_err(internal)?;
    if !ok {
        return Err((StatusCode::NOT_FOUND, "client not found"));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/admin/oauth-clients/{client_id}/reset-secret",
    summary = "Reset OAuth client secret (returns new plaintext)",
    params(("client_id" = String, Path,)),
    responses(
        (status = 200, body = ResetSecretResponse),
        (status = 404, description = "not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_reset_secret(
    State(state): State<TokenServerState>,
    Path(client_id): Path<String>,
) -> Result<Json<ResetSecretResponse>, (StatusCode, &'static str)> {
    let plain = oauth_client_svc::reset_secret(&state.db, &client_id)
        .await
        .map_err(internal)?
        .ok_or((StatusCode::NOT_FOUND, "client not found"))?;
    Ok(Json(ResetSecretResponse {
        client_id,
        client_secret: plain,
    }))
}
