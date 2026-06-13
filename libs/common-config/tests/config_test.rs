use common_config::Loader;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Demo { name: String, port: u16 }

#[test]
fn loader_reads_env_first() {
    std::env::set_var("DEMO_NAME", "envname");
    std::env::set_var("DEMO_PORT", "9999");
    let cfg: Demo = Loader::new("DEMO").load().unwrap();
    assert_eq!(cfg, Demo { name: "envname".into(), port: 9999 });
}