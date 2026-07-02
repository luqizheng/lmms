use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

use crate::routes::{access, admin, api_key, refresh, revoke, user_jwt};

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
        admin::handle_list,
        admin::handle_create,
        admin::handle_update,
        admin::handle_delete,
        admin::handle_reset_secret,
        api_key::handle_create_api_key,
        api_key::handle_list_api_keys,
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
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "tokens", description = "Token issuance"),
        (name = "admin", description = "OAuth client admin"),
        (name = "api-key", description = "Long-lived API key (client_credentials only)"),
    )
)]
pub struct ApiDoc;
