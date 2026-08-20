//! 单测：HandlerError display + get_redelivery_count 各 header 类型解析。
//! 不起 RabbitMQ，纯单元测。

use digua_common_event::HandlerError;
use lapin::types::{AMQPValue, FieldTable, LongString, ShortString};

#[test]
fn handler_error_transient_display() {
    let e = HandlerError::Transient("db timeout".into());
    assert_eq!(e.to_string(), "transient: db timeout");
}

#[test]
fn handler_error_permanent_display() {
    let e = HandlerError::Permanent("invalid payload".into());
    assert_eq!(e.to_string(), "permanent: invalid payload");
}

fn make_headers(entries: Vec<(&str, AMQPValue)>) -> Option<FieldTable> {
    let mut h = FieldTable::default();
    for (k, v) in entries {
        h.insert(ShortString::from(k), v);
    }
    Some(h)
}

#[test]
fn count_zero_when_header_absent() {
    let headers: Option<FieldTable> = None;
    assert_eq!(
        digua_common_event::consumer::get_redelivery_count(&headers),
        0
    );
}

#[test]
fn count_zero_when_header_field_absent() {
    let headers = make_headers(vec![("other", AMQPValue::LongLongInt(99))]);
    assert_eq!(
        digua_common_event::consumer::get_redelivery_count(&headers),
        0
    );
}

#[test]
fn count_parses_long_long_int() {
    let headers = make_headers(vec![("x-redelivery-count", AMQPValue::LongLongInt(2))]);
    assert_eq!(
        digua_common_event::consumer::get_redelivery_count(&headers),
        2
    );
}

#[test]
fn count_parses_long_int() {
    let headers = make_headers(vec![("x-redelivery-count", AMQPValue::LongInt(3))]);
    assert_eq!(
        digua_common_event::consumer::get_redelivery_count(&headers),
        3
    );
}

#[test]
fn count_zero_on_unexpected_type() {
    let headers = make_headers(vec![(
        "x-redelivery-count",
        AMQPValue::LongString(LongString::from("not-a-number")),
    )]);
    assert_eq!(
        digua_common_event::consumer::get_redelivery_count(&headers),
        0
    );
}
