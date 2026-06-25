use utoipa::OpenApi;

use crate::{
    models::{
        config_item::{
            ConfigItem, CreateConfigItem, PagedConfigItems, UpdateConfigItem,
        },
        namespace::{
            CreateNamespace, Namespace, PagedNamespaces, UpdateNamespace,
        },
    },
    routes::{config as cfg_route, namespace as ns_route, watch},
};

#[derive(OpenApi)]
#[openapi(
    info(title = "config-service", description = "Dynamic configuration management"),
    paths(
        ns_route::create_namespace,
        ns_route::query_namespaces,
        ns_route::get_namespace,
        ns_route::update_namespace,
        ns_route::delete_namespace,
        cfg_route::create_config_item,
        cfg_route::query_config_items,
        cfg_route::get_config_item,
        cfg_route::update_config_item,
        cfg_route::delete_config_item,
        cfg_route::export_config,
        cfg_route::import_config,
        watch::watch_config,
    ),
    components(schemas(
        Namespace,
        CreateNamespace,
        UpdateNamespace,
        PagedNamespaces,
        ConfigItem,
        CreateConfigItem,
        UpdateConfigItem,
        PagedConfigItems,
    )),
    tags(
        (name = "namespace", description = "Namespace management"),
        (name = "config-item", description = "Config item CRUD + export/import"),
        (name = "watch", description = "Long-poll watch"),
    )
)]
pub struct ApiDoc;
