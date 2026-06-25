use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(title = "audit-service", description = "Audit log and retention"),
    tags(
        (name = "audit", description = "Audit logs"),
    )
)]
pub struct ApiDoc;
