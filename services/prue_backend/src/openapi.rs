use utoipa::OpenApi;

use crate::{
    dto::{
        CreateMenuConfigRequest, MenuConfigDetail, MenuConfigListResponse, MenuConfigSummary,
        UpdateMenuConfigRequest,
    },
    routes::menu_configs,
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
    ),
    components(schemas(
        CreateMenuConfigRequest,
        UpdateMenuConfigRequest,
        MenuConfigDetail,
        MenuConfigSummary,
        MenuConfigListResponse,
    )),
    tags(
        (name = "menu-configs", description = "Menu config versions"),
    )
)]
pub struct ApiDoc;
