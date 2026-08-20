//! 注入逻辑测：fill_trace_id 的三个分支。
//! 不起 OTel、不起 MySQL，纯单元测分支行为。

use digua_common_event::envelope::Envelope;
use digua_common_event::event::Event;

// 测试 fixture payload
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
struct TestPayload;

impl Event for TestPayload {
    fn event_type() -> &'static str {
        "test.payload"
    }
    fn schema_version() -> u32 {
        1
    }
}

#[test]
fn fills_trace_id_when_none_and_source_some() {
    let mut env = Envelope::new("test", TestPayload);
    assert!(
        env.trace_id.is_none(),
        "precondition: fresh envelope has no trace_id"
    );

    // 通过 outbox 的内部 fill_trace_id，注入 fake source
    digua_common_event::outbox::fill_trace_id(&mut env, || Some("test-trace-id".into()));

    assert_eq!(env.trace_id.as_deref(), Some("test-trace-id"));
}

#[test]
fn respects_caller_trace_id_when_already_set() {
    let mut env = Envelope::new("test", TestPayload).with_trace_id("caller-fixed-id");

    // source 即便返 Some 也不应覆盖
    digua_common_event::outbox::fill_trace_id(&mut env, || Some("should-be-ignored".into()));

    assert_eq!(env.trace_id.as_deref(), Some("caller-fixed-id"));
}

#[test]
fn leaves_none_when_source_returns_none() {
    let mut env = Envelope::new("test", TestPayload);

    digua_common_event::outbox::fill_trace_id(&mut env, || None);

    assert!(
        env.trace_id.is_none(),
        "expected None when source returns None"
    );
}
