use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(title = "file-service", description = "File upload, download and presign"),
    tags(
        (name = "file", description = "File endpoints"),
    )
)]
pub struct ApiDoc;
