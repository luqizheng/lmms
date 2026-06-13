use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "api-gateway",
    version,
    about = "API gateway with multi-file routing"
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Parse and check routes/*.router.toml; exit 0 on success.
    Validate {
        #[arg(long, default_value = "config/api-gateway/routes")]
        routes: PathBuf,
    },
    /// Run the gateway.
    Serve {
        #[arg(long, default_value = "/app/config/routes")]
        routes: PathBuf,
        /// Admin HTTP port for POST /__admin/reload; 0 disables.
        #[arg(long, default_value_t = 9090)]
        admin_port: u16,
    },
}
