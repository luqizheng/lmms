use jwt_simple::prelude::*;

fn main() {
    let key_pair = RS256KeyPair::generate(2048).expect("failed to generate RSA key pair");

    let private_pem = key_pair.to_pem().expect("failed to encode private key");
    std::fs::write("keys/private.pem", private_pem.as_bytes())
        .expect("failed to write private key");

    let public_key = key_pair.public_key();
    let public_pem = public_key.to_pem().expect("failed to encode public key");
    std::fs::write("keys/public.pem", public_pem.as_bytes())
        .expect("failed to write public key");

    println!("RSA key pair generated in keys/");
    println!("  private: keys/private.pem");
    println!("  public:  keys/public.pem");
}
