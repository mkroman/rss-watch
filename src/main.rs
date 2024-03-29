use clap::Parser;
use directories::ProjectDirs;
use miette::{bail, Diagnostic, Result};
use thiserror::Error;
use tracing::debug;

use std::path::Path;
use std::thread;

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
    #[error("Script `{0}' is not executable")]
    ScriptNotExecutable(String),
    #[error(transparent)]
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

fn init_tracing() -> Result<()> {
    tracing_subscriber::fmt::init();

    Ok(())
}

fn main() -> Result<()> {
    init_tracing()?;

    let proj_dirs = ProjectDirs::from("dk.maero", "mkroman", "rss-watch")
        .expect("could not get user project dirs");
    let opts = cli::Opts::parse();

    let feed_url = opts.url;
    let database_url = {
        let default_database_path = proj_dirs.data_local_dir().join("database.db");
        opts.database_path.unwrap_or(default_database_path)
    };
    let scripts: Vec<&Path> = opts.scripts.iter().map(|x| x.as_path()).collect();

    if let Some(path) = scripts.iter().find(|e| !is_executable(e)) {
        bail!(CliError::ScriptNotExecutable((*path).display().to_string()));
    }

    debug!("Feed URL: {}", feed_url);

    let refresh_interval = opts.refresh_interval;

    debug!("Refresh interval: {:?}", refresh_interval);

    let mut watcher = Watcher::new(feed_url, refresh_interval.into(), scripts);
    watcher.open_database(database_url)?;
    watcher.probe()?;

    if opts.import_only {
        watcher.process_feed(false)?;
        return Ok(());
    }

    loop {
        watcher.process_feed(true)?;

        thread::sleep(watcher.interval());
    }
}
