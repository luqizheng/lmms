use std::fs;
use token_service::openapi::ApiDoc;
use utoipa::OpenApi;

fn main() {
    let yaml = ApiDoc::openapi()
        .to_yaml()
        .expect("failed to serialize OpenAPI to YAML");
    fs::write("openapi.yaml", yaml).expect("failed to write openapi.yaml");
}
