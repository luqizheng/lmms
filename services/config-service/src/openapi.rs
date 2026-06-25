use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(title = "config-service", description = "Dynamic configuration management"),
    tags(
        (name = "config", description = "Configuration endpoints"),
    )
)]
pub struct ApiDoc;
