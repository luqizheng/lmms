use utoipa::OpenApi;

use crate::{
    dto::{
        CreateMenuConfigRequest, MenuConfigDetail, MenuConfigListResponse, MenuConfigSummary,
        UpdateMenuConfigRequest,
    },
    routes::{menu_configs, migration_state},
};

#[derive(OpenApi)]
#[openapi(
    info(title = "prue_backend", description = "pure-admin menu config backend"),
    paths(
        menu_configs::get_async_routes,
        menu_configs::list_menu_configs,
        menu_configs::create_menu_config,
        menu_configs::get_menu_config,
        menu_configs::update_menu_config,
        menu_configs::publish_menu_config,
        menu_configs::clone_menu_config,
        menu_configs::delete_menu_config,
        migration_state::handler,
    ),
    components(schemas(
        CreateMenuConfigRequest,
        UpdateMenuConfigRequest,
        MenuConfigDetail,
        MenuConfigSummary,
        MenuConfigListResponse,
        common_db::migration_state::MigrationStateResponse,
        common_db::migration_state::MigrationInfo,
    )),
    tags(
        (name = "menu-configs", description = "Menu config versions"),
        (name = "internal", description = "Internal service-to-service endpoints (service-jwt)"),
    )
)]
pub struct ApiDoc;
