use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

use crate::dto;
use crate::routes::{auth, internal_membership, internal_users, org_units, permissions, roles, tenants, users};

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
        auth::revoke_session,
        auth::register,
        auth::setup_status,
        users::list_users,
        users::create_user,
        users::get_user,
        users::update_user,
        users::delete_user,
        users::list_users_simple,
        users::get_user_claims,
        roles::assign_user_roles,
        roles::list_roles,
        roles::create_role,
        roles::get_role,
        roles::update_role,
        roles::delete_role,
        roles::assign_permissions,
        permissions::list_permissions,
        permissions::create_permission,
        permissions::get_permission,
        permissions::update_permission,
        permissions::delete_permission,
        org_units::list_org_units,
        org_units::create_org_unit,
        org_units::get_org_unit,
        org_units::update_org_unit,
        org_units::delete_org_unit,
        org_units::assign_users,
        tenants::register_tenant,
        internal_users::list_users_by_role,
        internal_users::list_users_by_dept,
        internal_membership::membership_bootstrap,
    ),
    components(schemas(
        dto::LoginRequest,
        dto::LoginResponse,
        dto::RefreshRequest,
        dto::RefreshResponse,
        dto::ChangePasswordRequest,
        dto::ResetPasswordRequest,
        dto::UserInfo,
        dto::SessionInfo,
        dto::CreateUserRequest,
        dto::UpdateUserRequest,
        dto::UserResponse,
        dto::UserListResponse,
        dto::UserSimpleResponse,
        dto::UserSimpleListResponse,
        dto::UserClaimsResponse,
        dto::BatchImportUsersRequest,
        dto::CreateRoleRequest,
        dto::UpdateRoleRequest,
        dto::RoleResponse,
        dto::RoleListResponse,
        dto::AssignPermissionsRequest,
        dto::AssignUserRolesRequest,
        dto::AssignUserRolesResponse,
        dto::CreateTenantRequest,
        dto::CreateTenantResponse,
        dto::UserIdListResponse,
        dto::OrgUnitResponse,
        dto::CreateOrgUnitRequest,
        dto::UpdateOrgUnitRequest,
        dto::AssignUsersRequest,
        dto::PermissionResponse,
        dto::PermissionListResponse,
        dto::CreatePermissionRequest,
        dto::UpdatePermissionRequest,
        dto::PermissionQuery,
        internal_membership::MembershipBootstrap,
        internal_membership::UserMembership,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "auth", description = "Authentication"),
        (name = "users", description = "User management"),
        (name = "roles", description = "Role management"),
        (name = "permissions", description = "Permission management"),
        (name = "org_units", description = "Organization unit management"),
        (name = "tenants", description = "Tenant registration"),
        (name = "internal", description = "Internal service-to-service endpoints"),
    )
)]
pub struct ApiDoc;
