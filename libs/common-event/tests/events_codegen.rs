use common_event::events::{IamLoginSucceeded, IamUserCreated, IamUserDeleted};
use common_event::routing_keys;
use common_event::Envelope;

#[test]
fn routing_keys_match_yaml() {
    assert_eq!(routing_keys::IAM_USER_CREATED, "iam.user.created");
    assert_eq!(routing_keys::IAM_USER_DELETED, "iam.user.deleted");
    assert_eq!(routing_keys::IAM_LOGIN_SUCCEEDED, "iam.login.succeeded");
}

#[test]
fn iam_user_created_event_type() {
    let env = Envelope::new(
        "iam-service",
        IamUserCreated {
            user_id: "u1".to_string(),
            username: "alice".to_string(),
            email: Some("a@x".to_string()),
            org_unit_id: None,
        },
    );
    assert_eq!(env.event_type, "iam.user.created");
    let v = serde_json::to_value(&env).unwrap();
    assert_eq!(v["payload"]["username"], "alice");
    assert_eq!(v["payload"]["org_unit_id"], serde_json::Value::Null);
}

#[test]
fn iam_user_deleted_requires_deleted_by() {
    let env = Envelope::new(
        "iam-service",
        IamUserDeleted {
            user_id: "u1".to_string(),
            deleted_by: "admin".to_string(),
        },
    );
    assert_eq!(env.event_type, "iam.user.deleted");
    let v = serde_json::to_value(&env).unwrap();
    assert_eq!(v["payload"]["deleted_by"], "admin");
}

#[test]
fn iam_login_succeeded_round_trip() {
    let env = Envelope::new(
        "iam-service",
        IamLoginSucceeded {
            user_id: "u1".to_string(),
            ip: "10.0.0.1".to_string(),
            user_agent: Some("Mozilla".to_string()),
            method: "password".to_string(),
        },
    );
    let s = serde_json::to_string(&env).unwrap();
    let back: Envelope<IamLoginSucceeded> = serde_json::from_str(&s).unwrap();
    assert_eq!(back.payload.method, "password");
    assert_eq!(back.payload.user_agent, Some("Mozilla".to_string()));
}

#[test]
fn iam_login_failed_codegen_present() {
    use common_event::event::Event;
    use common_event::events::IamLoginFailed;
    assert_eq!(IamLoginFailed::event_type(), "iam.login.failed");
    assert_eq!(IamLoginFailed::schema_version(), 1);
    assert_eq!(common_event::routing_keys::IAM_LOGIN_FAILED, "iam.login.failed");
    let _ = IamLoginFailed {
        username: "u".into(),
        user_id: Some("id".into()),
        ip: "1.2.3.4".into(),
        user_agent: None,
        reason: "bad_password".into(),
    };
}
