use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

use crate::routes::{
    access, admin, api_key, bootstrap_token_admin, bootstrap_token_web, migration_state, refresh,
    revoke, user_jwt,
};

pub struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi
            .components
            .get_or_insert_with(utoipa::openapi::Components::new);
        components.add_security_scheme(
            "bearer_auth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        );
    }
}

#[derive(OpenApi)]
#[openapi(
    info(title = "token-service", description = "Token issuance and OAuth client admin"),
    paths(
        access::handle_access,
        refresh::handle_refresh,
        revoke::handle_revoke,
        user_jwt::handle_user_jwt,
        migration_state::handler,
        admin::handle_list,
        admin::handle_create,
        admin::handle_update,
        admin::handle_delete,
        admin::handle_reset_secret,
        api_key::handle_create_api_key,
        api_key::handle_list_api_keys,
        bootstrap_token_admin::handle_list,
        bootstrap_token_admin::handle_get,
        bootstrap_token_admin::handle_create,
        bootstrap_token_admin::handle_rotate,
        bootstrap_token_admin::handle_set_status,
        bootstrap_token_admin::handle_delete,
        bootstrap_token_web::handle_list,
        bootstrap_token_web::handle_get,
        bootstrap_token_web::handle_create,
        bootstrap_token_web::handle_rotate,
        bootstrap_token_web::handle_set_status,
        bootstrap_token_web::handle_delete,
    ),
    components(schemas(
        access::AccessRequest,
        access::AccessResponse,
        refresh::RefreshRequest,
        refresh::RefreshResponse,
        revoke::RevokeRequest,
        revoke::RevokeResponse,
        user_jwt::UserJwtRequest,
        user_jwt::UserJwtResponse,
        digua_common_db::migration_state::MigrationStateResponse,
        digua_common_db::migration_state::MigrationInfo,
        admin::ListResponse,
        admin::ClientDto,
        admin::CreateRequest,
        admin::CreateResponse,
        admin::UpdateRequest,
        admin::ResetSecretResponse,
        api_key::ApiKeyCreated,
        api_key::CreateApiKeyRequest,
        api_key::ListApiKeysResponse,
        api_key::ApiKeySummary,
        bootstrap_token_admin::BootstrapTokenDto,
        bootstrap_token_admin::ListBootstrapTokensResponse,
        bootstrap_token_admin::CreateBootstrapTokenRequest,
        bootstrap_token_admin::BootstrapTokenCreated,
        bootstrap_token_admin::RotateBootstrapTokenResponse,
        bootstrap_token_admin::SetStatusRequest,
        bootstrap_token_web::RotateRequest,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "tokens", description = "Token issuance"),
        (name = "admin", description = "OAuth client admin"),
        (name = "api-key", description = "Long-lived API key (client_credentials only)"),
        (
            name = "bootstrap-token",
            description = "Service-to-service credential management (Web UI)"
        ),
    )
)]
pub struct ApiDoc;
