use utoipa::OpenApi;

use crate::{models, rest};

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
        rest::read_one,
        rest::unread_count,
    ),
    components(
        schemas(
            models::Announcement,
            models::CreateAnnouncement,
            models::ScopeInput,
            models::Status,
            models::ScopeType,
            rest::ListQuery,
        )
    ),
    tags(
        (name = "announcements", description = "公告管理"),
        (name = "notifications", description = "通知管理"),
    )
)]
pub struct ApiDoc;
