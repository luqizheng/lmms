# Phase 3: digua_common-http / digua_common-crypto 完整化

> **STATUS: ✅ 已完成** — `libs/digua_common-http/` 和 `libs/digua_common-crypto/` 均存在，
> 被 api-gateway, iam-service, notification-service, token-service 引用。
> 本文为设计参考，不需进一步行动。

> 完成日期：2026-06-14

## digua_common-http

### 模块结构

```
libs/digua_common-http/src/
├─ lib.rs              # 导出
├─ client.rs           # HttpClient 封装
├─ retry.rs           # 重试策略（指数退避 + 抖动）
├─ circuit_breaker.rs  # 熔断器（Closed/Open/HalfOpen）
├─ rate_limit.rs      # 令牌桶限流
└─ service_discovery.rs # Consul 服务发现
```

### HttpClient

```rust
use digua_common_http::{HttpClient, HttpClientConfig};

let client = HttpClient::builder()
    .timeout(Duration::from_secs(10))
    .connect_timeout(Duration::from_secs(5))
    .pool_max_idle(10)
    .request_id_header("X-Request-Id")
    .build()?;
```

### RetryPolicy

```rust
use digua_common_http::RetryPolicy;

let policy = RetryPolicy::new(3)
    .base_delay(Duration::from_millis(200))
    .max_delay(Duration::from_secs(10))
    .jitter(0.1);

let result = policy.execute(|| async { Ok(()) }).await?;
```

### CircuitBreaker

```rust
use digua_common_http::{CircuitBreaker, CircuitState};

let cb = CircuitBreaker::new(5, Duration::from_secs(60))
    .half_open_max_calls(3);

if cb.is_available() {
    // call service
    cb.record_success();
} else {
    cb.record_failure();
}
```

### RateLimiter

```rust
use digua_common_http::RateLimiter;

let limiter = RateLimiter::new(100, 10); // capacity=100, refill=10/s

if limiter.try_acquire() {
    // proceed
}
```

### ConsulResolver

```rust
use digua_common_http::ConsulResolver;

let resolver = ConsulResolver::new("http://localhost:8500")
    .cache_ttl(30);

let url = resolver.resolve_url("user-service").await?;
```

---

## digua_common-crypto

### 模块结构

```
libs/digua_common-crypto/src/
├─ lib.rs              # 导出
├─ aead.rs            # AES-256-GCM（已存在）
├─ password.rs        # Argon2id（已存在）
├─ rsa.rs             # RSA-OAEP / RSA-PKCS1
├─ hmac.rs            # HMAC-SHA256 / HMAC-SHA512
├─ kms.rs             # KMS trait + LocalKms
└─ hash.rs            # SHA-256 / BLAKE3
```

### AEAD (AES-256-GCM)

```rust
use digua_common_crypto::{AeadCipher, Aes256Gcm};

let cipher = Aes256Gcm::new(key);
let ciphertext = cipher.encrypt(plaintext)?;
let plaintext = cipher.decrypt(&ciphertext)?;
```

### Password (Argon2id)

```rust
use digua_common_crypto::PasswordHasher;

let hasher = PasswordHasher::default();
let hash = hasher.hash(password)?;
assert!(hasher.verify(password, &hash)?);
```

### RSA

```rust
use digua_common_crypto::{RsaKeyPair, RsaPublicKey, RsaScheme};

// 生成
let keypair = RsaKeyPair::generate(2048)?;

// 加密/解密
let ciphertext = keypair.encrypt(plaintext, RsaScheme::OaepSha256)?;
let decrypted = keypair.decrypt(&ciphertext, RsaScheme::OaepSha256)?;

// 签名/验签
let signature = keypair.sign(message, RsaScheme::Pkcs1Sha256)?;
let pub_key = RsaPublicKey::from_pem(&keypair.public_key_pem()?)?;
assert!(pub_key.verify(message, &signature, RsaScheme::Pkcs1Sha256)?);
```

### HMAC

```rust
use digua_common_crypto::{HmacSha256, HmacSha512};

let hmac = HmacSha256::new(b"secret");
let digest = hmac.mac(b"secret", b"message");
assert!(hmac.verify(b"secret", b"message", &digest));
```

### KMS

```rust
use digua_common_crypto::{KmsClient, LocalKms};

#[async_trait]
trait KmsClient {
    async fn encrypt(&self, key_id: &str, plaintext: &[u8]) -> Result<Vec<u8>>;
    async fn decrypt(&self, key_id: &str, ciphertext: &[u8]) -> Result<Vec<u8>>;
    async fn generate_data_key(&self, key_id: &str) -> Result<DataKey>;
}

let kms = LocalKms::new(master_key);
let ciphertext = kms.encrypt("my-key", b"secret").await?;
let plaintext = kms.decrypt("my-key", &ciphertext).await?;
```

### Hash

```rust
use digua_common_crypto::{Sha256, Blake3};

let hash = Sha256::hash(b"data");
let hex = Sha256::hash_hex(b"data");

let hash = Blake3::hash(b"data");
let hex = Blake3::hash_hex(b"data");
```

---

## 测试

```bash
cargo test -p digua_common-http
cargo test -p digua_common-crypto
```
