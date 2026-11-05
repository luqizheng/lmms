use utoipa::OpenApi;

use crate::{migration_state_route, models, rest};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Notification Service",
        version = "1.0.0",
        description = "Notification Service for coder-micro-services"
    ),
    paths(
        rest::create,
        rest::publish,
        rest::revoke,
        rest::detail,
        rest::list_mine,
        rest::list_all,
        rest::read_one,
        rest::read_all,
        rest::unread_count,
        migration_state_route::handler,
    ),
    components(
        schemas(
            models::Announcement,
            models::CreateAnnouncement,
            models::ScopeInput,
            models::Status,
            models::ScopeType,
            rest::ListQuery,
            digua_common_db::migration_state::MigrationStateResponse,
            digua_common_db::migration_state::MigrationInfo,
        )
    ),
    tags(
        (name = "announcements", description = "公告管理"),
        (name = "notifications", description = "通知管理"),
        (name = "internal", description = "Internal service-to-service endpoints (service-jwt)"),
    )
)]
pub struct ApiDoc;
