use utoipa::OpenApi;

use crate::{
    models::{
        login::{LoginLog, PagedLoginLogs},
        operation::{OperationLog, PagedOperationLogs},
        retention::{DataStage, RetentionPolicy, RetentionStats},
    },
    routes::{compliance, login, operation, retention},
};

#[derive(OpenApi)]
#[openapi(
    info(title = "audit-service", description = "Audit log and retention"),
    paths(
        operation::query_operation_logs,
        login::query_login_logs,
        compliance::export_operation_csv,
        compliance::export_operation_json,
        compliance::export_login_csv,
        retention::get_retention_stats,
        retention::get_retention_policy,
        retention::update_retention_policy,
        retention::trigger_cleanup,
        retention::get_data_stage,
    ),
    components(schemas(
        OperationLog,
        PagedOperationLogs,
        LoginLog,
        PagedLoginLogs,
        RetentionPolicy,
        RetentionStats,
        DataStage,
    )),
    tags(
        (name = "operation", description = "Operation logs"),
        (name = "login", description = "Login logs"),
        (name = "compliance", description = "Compliance export"),
        (name = "retention", description = "Retention policy & cleanup"),
    )
)]
pub struct ApiDoc;
