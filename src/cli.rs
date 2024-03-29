//! CLI support and integration

use std::path::PathBuf;

use clap::Parser;
use url::Url;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Opts {
    /// Refresh interval in seconds
    #[arg(long, short, env = "REFRESH_INTERVAL", default_value = "60s")]
    pub refresh_interval: humantime::Duration,
    /// Path to the database file
    #[arg(long, short, env = "DATABASE_PATH")]
    pub database_path: Option<PathBuf>,
    /// Read entries from feed and persist them to the database without executing scripts, and then
    /// exit
    #[arg(long, default_value_t = false)]
    pub import_only: bool,
    /// RSS or Atom feed URL
    pub url: Url,
    /// Scripts to execute when there's new entries in the feed
    pub scripts: Vec<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;

        Opts::command().debug_assert()
    }
}
