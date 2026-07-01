use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

use crate::dto;
use crate::routes::{auth, internal_users, tenants, users};

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
    info(title = "iam-service", description = "Identity, role and user management"),
    paths(
        auth::login,
        auth::refresh,
        auth::logout,
        auth::change_password,
        auth::list_sessions,
        users::list_users,
        users::create_user,
        users::get_user,
        users::update_user,
        users::delete_user,
        tenants::register_tenant,
        internal_users::list_users_by_role,
        internal_users::list_users_by_dept,
    ),
    components(schemas(
        dto::LoginRequest,
        dto::LoginResponse,
        dto::RefreshRequest,
        dto::RefreshResponse,
        dto::ChangePasswordRequest,
        dto::UserInfo,
        dto::SessionInfo,
        dto::CreateUserRequest,
        dto::UpdateUserRequest,
        dto::UserResponse,
        dto::UserListResponse,
        dto::CreateTenantRequest,
        dto::CreateTenantResponse,
        dto::UserIdListResponse,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "auth", description = "Authentication"),
        (name = "users", description = "User management"),
        (name = "tenants", description = "Tenant registration"),
        (name = "internal", description = "Internal service-to-service endpoints"),
    )
)]
pub struct ApiDoc;
