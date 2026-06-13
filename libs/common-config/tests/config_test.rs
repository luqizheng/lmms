use common_config::Loader;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Default)]
struct Demo {
    name: String,
    port: u16,
}

#[test]
fn loader_reads_env() {
    std::env::set_var("TEST_NAME", "envname");
    std::env::set_var("TEST_PORT", "9999");
    let cfg: Demo = Loader::new("TEST").load().unwrap();
    assert_eq!(
        cfg,
        Demo {
            name: "envname".into(),
            port: 9999
        }
    );
}

#[test]
fn loader_with_nonexistent_file() {
    let cfg: Demo = Loader::new("NOPE")
        .with_file("non-existent-file.toml")
        .load()
        .unwrap_or_else(|_| Demo {
            name: "default".into(),
            port: 8080,
        });
    assert_eq!(cfg.name, "default");
}

#[derive(Debug, Deserialize, PartialEq, Default)]
struct OptionalConfig {
    #[serde(default)]
    host: String,
    #[serde(default = "default_port")]
    port: u16,
}

fn default_port() -> u16 {
    8080
}

#[test]
fn loader_with_default_values() {
    std::env::remove_var("OPT_HOST");
    std::env::remove_var("OPT_PORT");
    let cfg: OptionalConfig = Loader::new("OPT").load().unwrap();
    assert_eq!(cfg.host, "");
    assert_eq!(cfg.port, 8080);
}
