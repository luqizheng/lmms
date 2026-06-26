use common_event::{Envelope, Event};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TestEvent {
    user_id: String,
}

impl Event for TestEvent {
    fn event_type() -> &'static str { "test.event" }
    fn schema_version() -> u32 { 1 }
}

#[test]
fn envelope_new_fills_event_id_and_occurred_at() {
    let before = chrono::Utc::now();
    let env = Envelope::new("test-producer", TestEvent { user_id: "u1".to_string() });
    let after = chrono::Utc::now();

    assert_eq!(env.event_type, "test.event");
    assert_eq!(env.payload.user_id, "u1");
    assert_eq!(env.producer, "test-producer");
    assert!(env.tenant_id.is_none());
    assert!(env.trace_id.is_none());
    assert!(env.occurred_at >= before && env.occurred_at <= after);
    // event_id is a valid UUIDv7 (version bits = 0b0111 in time_hi)
    let v = env.event_id.get_version_num();
    assert_eq!(v, 7, "event_id should be UUIDv7, got version {}", v);
}

#[test]
fn envelope_serializes_to_json_with_event_type() {
    let env = Envelope::new("p1", TestEvent { user_id: "u1".to_string() });
    let json = serde_json::to_value(&env).unwrap();
    assert_eq!(json["event_type"], "test.event");
    assert_eq!(json["producer"], "p1");
    assert_eq!(json["payload"]["user_id"], "u1");
    assert!(json["schema_version"].is_number());
}

#[test]
fn envelope_round_trip_through_json() {
    let env = Envelope::new("p1", TestEvent { user_id: "u1".to_string() });
    let json = serde_json::to_string(&env).unwrap();
    let back: Envelope<TestEvent> = serde_json::from_str(&json).unwrap();
    assert_eq!(back.payload, env.payload);
    assert_eq!(back.event_id, env.event_id);
}
