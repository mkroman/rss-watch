use clap::Parser;
use directories::ProjectDirs;
use miette::{bail, Diagnostic};
use thiserror::Error;
use tracing::debug;

use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

mod cli;
mod database;
mod error;
mod watcher;

pub use database::Database;
pub use error::Error;
pub use watcher::Watcher;

#[derive(Debug, Diagnostic, Error)]
enum CliError {
    #[error("interval is not specified")]
    MissingInterval,
    #[error("`{0}' is not executable")]
    ScriptNotExecutable(String),
    #[error("{0}")]
    WatcherError(#[from] Error),
}

#[cfg(unix)]
fn is_executable<P: AsRef<Path>>(path: P) -> bool {
    let Ok(metadata) = path.as_ref().metadata() else {
        return false;
    };
    let permissions = metadata.permissions();

    permissions.mode() & 0o111 != 0
}

#[cfg(target_os = "windows")]
fn is_executable<P: AsRef<Path>>(path: P) -> bool {
    unimplemented!();
}

fn init_tracing() -> miette::Result<()> {
    tracing_subscriber::fmt::init();

    Ok(())
}

fn main() -> miette::Result<()> {
    init_tracing()?;

    let proj_dirs =
        ProjectDirs::from("dk.maero", "", "rss-watch").expect("could not get user project dirs");

    let default_database_path = proj_dirs.data_local_dir().join("database.db");

    let opts = cli::Opts::parse();

    let feed_url = opts.url;
    let scripts: Vec<&Path> = opts.scripts.iter().map(|x| x.as_path()).collect();

    if let Some(path) = scripts.iter().find(|e| !is_executable(e)) {
        bail!(CliError::ScriptNotExecutable((*path).display().to_string()));
    }

    debug!("Feed URL: {}", feed_url);

    let interval = opts.refresh_interval.ok_or(CliError::MissingInterval)?;

    debug!("Update interval: {:?}", interval);

    let mut watcher = Watcher::new(feed_url, interval.into(), scripts);
    watcher.open_database(opts.database_path.unwrap_or(default_database_path))?;
    watcher.probe()?;

    if opts.import_only {
        watcher.process_feed(false)?;
    }

    loop {
        watcher.process_feed(true)?;

        std::thread::sleep(watcher.interval());
    }
}
