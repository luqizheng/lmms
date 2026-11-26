//! WP-TEST-01：blacklist Redis key 拼装契约测试（PURE，无任何基础设施）。
//!
//! `blacklist_key(prefix, jti)` 是 revoke 写入侧与 gateway 读取侧共享的
//! key 契约——格式漂移会让已撤销 token 通过鉴权。本文件锁死该契约。

use api_gateway::blacklist::blacklist_key;

#[test]
fn key_contract_is_prefix_blacklist_colon_jti() {
    assert_eq!(
        blacklist_key("test:api-gateway:blacklist:", "test-jti-blacklist-001"),
        "test:api-gateway:blacklist:blacklist:test-jti-blacklist-001"
    );
}

#[test]
fn key_contract_empty_prefix_yields_leading_blacklist_segment() {
    // prefix 以分隔符结尾是 gateway 现有配置约定（routes 里的 key_prefix）；
    // 空 prefix 时的退化输出也必须稳定（排查线上的 key 缺前缀问题时依赖）。
    assert_eq!(blacklist_key("", "jti-1"), "blacklist:jti-1");
}

#[test]
fn key_contract_keeps_special_chars_in_jti_verbatim() {
    // jti 来自 JWT claim（sub+uuid 等），内容不受本层校验——契约是"原样拼接"，
    // 任何转义/过滤都会造成写入侧与读取侧不一致。
    assert_eq!(
        blacklist_key("p:", "a/b+c=d.e~f"),
        "p:blacklist:a/b+c=d.e~f"
    );
}

#[test]
fn key_contract_different_prefixes_never_collide() {
    let jti = "shared-jti";
    let k1 = blacklist_key("test:prefix1:", jti);
    let k2 = blacklist_key("test:prefix2:", jti);
    assert_ne!(k1, k2, "不同 prefix 的黑名单条目必须互相隔离");
}
