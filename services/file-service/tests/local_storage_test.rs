use bytes::Bytes;
use file_service::storage::{LocalStorage, ObjectStore};

#[tokio::test]
async fn local_storage_delete_is_idempotent() {
    let dir = tempfile::tempdir().expect("temporary storage root");
    let storage = LocalStorage::new(dir.path());

    // Deleting an object that is already absent is a successful no-op.
    storage.delete("missing-object").await.expect("delete missing object");

    storage
        .put("object-1", Bytes::from_static(b"payload"))
        .await
        .expect("put object");
    storage.delete("object-1").await.expect("first delete");
    storage.delete("object-1").await.expect("repeated delete");
    assert!(!storage.exists("object-1").await.expect("exists check"));
}
