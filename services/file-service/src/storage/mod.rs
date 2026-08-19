pub mod local;
pub mod object_store;
pub mod s3;

pub use local::LocalStorage;
pub use object_store::ObjectStore;
pub use s3::{S3Config, S3Storage};
