use digua_common_log::request_id::RequestId;

#[test]
fn request_id_generates_uuid_when_absent() {
    let rid = RequestId::from_header(None);
    assert!(!rid.as_str().is_empty());
    assert_eq!(rid.as_str().len(), 36);
}

#[test]
fn request_id_passthrough_header() {
    let rid = RequestId::from_header(Some("abc-123"));
    assert_eq!(rid.as_str(), "abc-123");
}

#[test]
fn request_id_handles_empty_string_as_none() {
    let rid = RequestId::from_header(Some(""));
    assert!(!rid.as_str().is_empty());
    assert_eq!(rid.as_str().len(), 36);
}

#[test]
fn request_id_handles_whitespace_as_none() {
    let rid = RequestId::from_header(Some("   "));
    assert!(!rid.as_str().is_empty());
    assert_eq!(rid.as_str().len(), 36);
}

#[test]
fn request_id_is_clonable() {
    let rid = RequestId::from_header(Some("test-id"));
    let cloned = rid.clone();
    assert_eq!(rid.as_str(), cloned.as_str());
}
