use common_log::request_id::RequestId;

#[test]
fn request_id_generates_uuid_when_absent() {
    let rid = RequestId::from_header(None);
    assert!(!rid.as_str().is_empty());
}

#[test]
fn request_id_passthrough_header() {
    let rid = RequestId::from_header(Some("abc-123"));
    assert_eq!(rid.as_str(), "abc-123");
}