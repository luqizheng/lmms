use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "API Gateway",
        version = "1.0.0",
        description = "API Gateway for coder-micro-services. Proxies requests to downstream services. \
                      For per-service OpenAPI specs, see the respective service's Swagger UI."
    ),
    paths(),
    components()
)]
pub struct ApiDoc;
