use digua_common_jwt::cache::parse_jwks;
use serde_json::json;

#[test]
fn parse_jwks_loads_rsa_keys_with_kid() {
    let json = json!({
        "keys": [
            {
                "kid": "k1",
                "kty": "RSA",
                "n": "0vx7agoebGcQSuuPiLJXZptN9nndrQmbXEps2aiAFbWhM78LhWx4cbbfAAtVT86zwu1RK7aPFFxuhDR1L6tSoc_BJECPebWKRXjBZCiFV4n3oknjhMstn64tZ_2W-5JsGY4Hc5n9yBXArwl93lqt7_RN5w6Cf0h4QyQ5v-65YGjQR0_FDW2QvzqY368QQMicJta6bszkvDzcsAtx6o7n4Qe0g22J4VS6X2hw6uK6m4T7B3e3Q4Xb-w6uK3m4T7B3e3Q4Xb-w",
                "e": "AQAB",
            }
        ]
    }).to_string();
    let keys = parse_jwks(&json).unwrap();
    assert!(keys.contains_key("k1"));
}

#[test]
fn parse_jwks_skips_non_rsa_keys() {
    let json = json!({
        "keys": [
            {
                "kid": "rsa-key",
                "kty": "RSA",
                "n": "0vx7agoebGcQSuuPiLJXZptN9nndrQmbXEps2aiAFbWhM78LhWx4cbbfAAtVT86zwu1RK7aPFFxuhDR1L6tSoc_BJECPebWKRXjBZCiFV4n3oknjhMstn64tZ_2W-5JsGY4Hc5n9yBXArwl93lqt7_RN5w6Cf0h4QyQ5v-65YGjQR0_FDW2QvzqY368QQMicJta6bszkvDzcsAtx6o7n4Qe0g22J4VS6X2hw6uK6m4T7B3e3Q4Xb-w6uK3m4T7B3e3Q4Xb-w",
                "e": "AQAB",
            },
            {
                "kid": "ec-key",
                "kty": "EC",
                "crv": "P-256",
            }
        ]
    }).to_string();
    let keys = parse_jwks(&json).unwrap();
    assert!(keys.contains_key("rsa-key"));
    assert!(!keys.contains_key("ec-key"));
}

#[test]
fn parse_jwks_rejects_invalid_json() {
    let result = parse_jwks("not json");
    assert!(result.is_err());
}
