//! Key-lifecycle admin endpoints (`/admin/keys*`).
//!
//! Auth: service-jwt `aud=token-service` (same gate as
//! `/admin/bootstrap-tokens*`) — being able to rotate the signing key is
//! equivalent to being able to impersonate any user, so it must never be
//! exposed without that gate.
//!
//! Semantics (2026-11-23 key persistence spec):
//! - `POST /admin/keys/rotate` — new key becomes the only signer; previous
//!   active key → `retiring` (verify-only, still in JWKS). No forced logout.
//! - `POST /admin/keys/{kid}/retire` — `retiring` → `retired`; the key
//!   leaves the JWKS, so verifiers stop accepting tokens signed by it.
//!   Only allowed on `retiring` keys (the single active signer can never be
//!   retired directly).
//! - `DELETE /admin/keys/{kid}` — delete a `retired` key, but only once
//!   `min_retention_sec` (TOKEN_JWT_MIN_RETENTION_SEC, default 30 d) has
//!   elapsed since retirement, so no verifiable token is ever destroyed.
//!
//! Every mutation is persisted to the keystore file atomically and, on
//! success, re-installed into the in-process JWKS cache + issuer so the
//! change takes effect immediately (this service is its own JWKS authority).

use crate::TokenServerState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use digua_jwt_ctx::Principal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

fn now_sec() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Audit identity of the caller: JWT `sub` plus the request trace id (when
/// the telemetry layer produced one).
fn audit_ctx(principal: &Principal) -> crate::audit::AuditContext {
    crate::audit::AuditContext::new(
        principal.user_id.clone(),
        digua_common_telemetry::current_trace_id(),
    )
}

#[derive(Debug, Serialize, ToSchema)]
pub struct KeyEntryView {
    pub kid: String,
    pub status: String,
    pub created_at: i64,
    pub retired_at: Option<i64>,
    /// true for the key currently signing new tokens.
    pub active: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListKeysResponse {
    pub active_kid: String,
    pub min_retention_sec: i64,
    pub keys: Vec<KeyEntryView>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RotateKeyResponse {
    pub active_kid: String,
    pub previous_kid: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OkResponse {
    pub ok: bool,
    pub kid: String,
    pub status: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RotateKeyRequest {
    /// Caller-side guard: client echoes the kid it believes is active; a
    /// mismatch is a 409 (protects against double-rotate races). Optional.
    pub expect_active_kid: Option<String>,
}

fn entry_view(kid_now: &str, k: &crate::keystore::KeyEntry) -> KeyEntryView {
    KeyEntryView {
        kid: k.kid.clone(),
        status: match k.status {
            crate::keystore::KeyStatus::Active => "active".into(),
            crate::keystore::KeyStatus::Retiring => "retiring".into(),
            crate::keystore::KeyStatus::Retired => "retired".into(),
        },
        created_at: k.created_at,
        retired_at: k.retired_at,
        active: k.kid == kid_now,
    }
}

#[utoipa::path(
    get,
    path = "/admin/keys",
    summary = "List JWT signing keys (metadata only, never key material)",
    responses((status = 200, body = ListKeysResponse)),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_list(
    State(state): State<TokenServerState>,
) -> Result<Json<ListKeysResponse>, (StatusCode, &'static str)> {
    let ks = state.keys.read().expect("keystore lock poisoned");
    let active_kid = ks.active().kid.clone();
    let keys = ks
        .data()
        .keys
        .iter()
        .map(|k| entry_view(&active_kid, k))
        .collect();
    Ok(Json(ListKeysResponse {
        active_kid,
        min_retention_sec: ks.min_retention_sec(),
        keys,
    }))
}

#[utoipa::path(
    post,
    path = "/admin/keys/rotate",
    summary = "Rotate the signing key (previous key stays for verification)",
    request_body = RotateKeyRequest,
    responses(
        (status = 200, description = "rotated", body = RotateKeyResponse),
        (status = 409, description = "expect_active_kid mismatch or persistence failure"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_rotate(
    State(state): State<TokenServerState>,
    principal: Principal,
    body: Option<Json<RotateKeyRequest>>,
) -> Result<Json<RotateKeyResponse>, (StatusCode, String)> {
    let ctx = audit_ctx(&principal);
    let expected = body
        .as_ref()
        .and_then(|Json(b)| b.expect_active_kid.clone());

    let (new_kid, prev_kid) = {
        let mut ks = state.keys.write().expect("keystore lock poisoned");
        if let Some(exp) = &expected {
            if *exp != ks.active().kid {
                let current = ks.active().kid.clone();
                ks.audit_ledger().record(
                    &ctx,
                    crate::audit::AuditAction::Rotate,
                    crate::audit::AuditResult::Failed,
                    Some(&current),
                    None,
                    Some("expect_active_kid mismatch (caller holds a stale view)"),
                );
                return Err((
                    StatusCode::CONFLICT,
                    format!("expect_active_kid mismatch: keystore active kid is {current}"),
                ));
            }
        }
        ks.rotate(now_sec(), &ctx)
            .map_err(|e| (StatusCode::CONFLICT, e.to_string()))?
    };

    refresh_runtime_state(&state).await;
    tracing::info!(new_kid = %new_kid, previous_kid = %prev_kid, "JWT signing key rotated via admin API");
    Ok(Json(RotateKeyResponse {
        active_kid: new_kid,
        previous_kid: prev_kid,
    }))
}

#[utoipa::path(
    post,
    path = "/admin/keys/{kid}/retire",
    summary = "Retire a rotating key (leaves JWKS; verify-only keys only)",
    params(("kid" = String, Path,)),
    responses(
        (status = 200, body = OkResponse),
        (status = 404, description = "unknown kid"),
        (status = 409, description = "key not in 'retiring' status"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_retire(
    State(state): State<TokenServerState>,
    principal: Principal,
    Path(kid): Path<String>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let ctx = audit_ctx(&principal);
    {
        let mut ks = state.keys.write().expect("keystore lock poisoned");
        ks.retire(&kid, now_sec(), &ctx).map_err(|e| match e {
            crate::keystore::KeyStoreError::InvalidOperation(msg)
                if msg.contains("unknown kid") =>
            {
                (StatusCode::NOT_FOUND, msg)
            }
            _ => (StatusCode::CONFLICT, e.to_string()),
        })?;
    }
    refresh_runtime_state(&state).await;
    tracing::info!(kid = %kid, "JWT key retired via admin API");
    Ok(Json(OkResponse {
        ok: true,
        kid,
        status: "retired".into(),
    }))
}

#[utoipa::path(
    delete,
    path = "/admin/keys/{kid}",
    summary = "Delete a retired key (only after the retention period)",
    params(("kid" = String, Path,)),
    responses(
        (status = 200, body = OkResponse),
        (status = 404, description = "unknown kid"),
        (status = 409, description = "not retired yet or retention not satisfied"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn handle_delete(
    State(state): State<TokenServerState>,
    principal: Principal,
    Path(kid): Path<String>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let ctx = audit_ctx(&principal);
    {
        let mut ks = state.keys.write().expect("keystore lock poisoned");
        ks.remove(&kid, now_sec(), &ctx).map_err(|e| match e {
            crate::keystore::KeyStoreError::InvalidOperation(msg)
                if msg.contains("unknown kid") =>
            {
                (StatusCode::NOT_FOUND, msg)
            }
            _ => (StatusCode::CONFLICT, e.to_string()),
        })?;
    }
    refresh_runtime_state(&state).await;
    tracing::info!(kid = %kid, "JWT key deleted via admin API");
    Ok(Json(OkResponse {
        ok: true,
        kid,
        status: "deleted".into(),
    }))
}

/// Push the new keystore state into the two in-memory consumers:
/// 1. JWKS cache (inbound service-jwt verification, kid-strict);
/// 2. issuer (outbound signing — new active key + kid).
///
/// This service is its own JWKS authority, so HTTP refresh would be both
/// racy and redundant; installing directly keeps rotate atomic per request.
async fn refresh_runtime_state(state: &TokenServerState) {
    // Guard scope: std guards are !Send, so they must be confined to a
    // block that ends BEFORE any .await (a bare `drop(ks)` is not enough
    // for the async generator's analysis).
    let (entries, active_pem, active_kid) = {
        let ks = state.keys.read().expect("keystore lock poisoned");
        let entries: Vec<(String, String)> = ks
            .public_entries()
            .into_iter()
            .map(|k| (k.kid.clone(), k.public_pem.clone()))
            .collect();
        (
            entries,
            ks.active().private_pem.clone(),
            ks.active().kid.clone(),
        )
    };

    state
        .jwks
        .install_from_pems(&entries)
        .await
        .expect("re-install JWKS keys after keystore mutation");
    state
        .issuer
        .swap_active_key(&active_pem, &active_kid)
        .expect("swap issuer key after keystore mutation");
}

pub fn routes(state: crate::TokenServerState) -> axum::Router {
    use axum::routing::{get, post};

    axum::Router::new()
        .route("/admin/keys", get(handle_list))
        .route("/admin/keys/rotate", post(handle_rotate))
        .route("/admin/keys/{kid}/retire", post(handle_retire))
        .route("/admin/keys/{kid}", axum::routing::delete(handle_delete))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            digua_authz::verify_aud_principal_middleware::<crate::TokenServerState>(
                "token-service",
            ),
        ))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_view_marks_active_and_maps_status() {
        use crate::keystore::KeyStatus;
        let e = crate::keystore::KeyEntry {
            kid: "abc".into(),
            status: KeyStatus::Retiring,
            created_at: 1,
            retired_at: None,
            private_pem: "x".into(),
            public_pem: "y".into(),
        };
        let v = entry_view("other", &e);
        assert_eq!(v.status, "retiring");
        assert!(!v.active);
        let v2 = entry_view("abc", &e);
        assert!(v2.active);
    }
}
