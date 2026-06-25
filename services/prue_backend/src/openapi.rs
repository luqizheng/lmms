use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(title = "prue_backend", description = "pure-admin menu config backend"),
    tags(
        (name = "menu-configs", description = "Menu config versions"),
    )
)]
pub struct ApiDoc;
