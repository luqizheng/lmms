use utoipa::OpenApi;

use crate::{
    models::{
        file_info::{FileInfo, PagedFiles, UploadFileMeta},
        presign_url::{CreatePresignUrl, PresignOperation, PresignUrlResponse},
    },
    routes::{file as file_route, migration_state, presign},
};

#[derive(OpenApi)]
#[openapi(
    info(title = "file-service", description = "File upload, download and presign"),
    paths(
        file_route::upload_file,
        file_route::query_files,
        file_route::download_file,
        file_route::delete_file,
        file_route::get_file_info,
        presign::create_presign,
        migration_state::handler,
    ),
    components(schemas(
        FileInfo,
        UploadFileMeta,
        PagedFiles,
        CreatePresignUrl,
        PresignUrlResponse,
        digua_common_db::migration_state::MigrationStateResponse,
        digua_common_db::migration_state::MigrationInfo,
    )),
    tags(
        (name = "file", description = "File upload / download / info"),
        (name = "presign", description = "Presigned URL"),
    )
)]
pub struct ApiDoc;
