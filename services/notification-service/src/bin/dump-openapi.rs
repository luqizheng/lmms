use utoipa::OpenApi;
use notification_service::openapi::ApiDoc;
use std::fs;

fn main() {
    let yaml = ApiDoc::openapi()
        .to_yaml()
        .expect("failed to serialize OpenAPI to YAML");
    fs::write("openapi.yaml", yaml).expect("failed to write openapi.yaml");
}
