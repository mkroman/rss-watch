use app_dirs::{get_app_dir, AppDataType, AppInfo};
use clap::{crate_authors, crate_version, App, AppSettings, Arg};
use failure::Fail;
use log::debug;

use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

mod database;
mod error;
mod watcher;

pub use database::Database;
pub use error::Error;
pub use watcher::Watcher;

const APP_INFO: AppInfo = AppInfo {
    name: "rss-watch",
    author: "Mikkel Kroman",
};

#[derive(Debug, Fail)]
enum CliError {
    #[fail(display = "interval is not specified")]
    MissingInterval,
    #[fail(display = "`{}' is not executable", _0)]
    ScriptNotExecutable(String),
    #[fail(display = "{}", error)]
    WatcherError {
        #[fail(cause)]
        error: Error,
    },
}

impl From<error::Error> for CliError {
    fn from(error: error::Error) -> Self {
        CliError::WatcherError { error }
    }
}

#[cfg(unix)]
fn is_executable<P: AsRef<Path>>(path: P) -> bool {
    let Ok(metadata) = path.as_ref().metadata() else { return false };
    let permissions = metadata.permissions();

    permissions.mode() & 0o111 != 0
}

#[cfg(target_os = "windows")]
fn is_executable<P: AsRef<Path>>(path: P) -> bool {
    unimplemented!();
}

fn main() {
    match try_main() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Error: {e} - {e:?}");
        }
    }
}

fn try_main() -> Result<(), CliError> {
    env_logger::init();

    let default_database_path =
        get_app_dir(AppDataType::UserData, &APP_INFO, "database.db").unwrap();

    let matches = App::new("rss-watch")
        .setting(AppSettings::ColoredHelp)
        .version(crate_version!())
        .author(crate_authors!("\n"))
        .about("Scriptable RSS/Atom feed watching tool")
        .arg(
            Arg::with_name("interval")
                .short("i")
                .long("interval")
                .help("Feed refresh interval in seconds")
                .default_value("1h")
                .validator(|input| {
                    humantime::parse_duration(&input)
                        .map(|_| ())
                        .map_err(|err| format!("Could not parse interval: {err}"))
                })
                .value_name("INTERVAL")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("database")
                .short("d")
                .long("database")
                .help("Database file path")
                .value_name("DATABASE")
                .default_value(default_database_path.to_str().unwrap()),
        )
        .arg(Arg::with_name("init")
             .long("init")
             .help("Saves the entries to the database without executing scripts on the first pass.")
             .takes_value(false))
        .arg(Arg::with_name("url").value_name("URL").required(true))
        .arg(
            Arg::with_name("scripts")
                .required(true)
                .multiple(true)
                .value_name("SCRIPT")
                .help("Program to execute when there's new entries in the feed. Can be specified multiple times.")
        )
        .get_matches();

    let feed_url = matches.value_of("url").unwrap();
    let scripts: Vec<&str> = matches.values_of("scripts").unwrap_or_default().collect();

    if let Some(path) = scripts.iter().find(|e| !is_executable(e)) {
        return Err(CliError::ScriptNotExecutable((*path).to_string()));
    }

    debug!("Feed URL: {}", feed_url);

    let interval = matches
        .value_of("interval")
        .ok_or(CliError::MissingInterval)
        .map(|interval| humantime::parse_duration(interval).unwrap())?;

    debug!("Update interval: {:?}", interval);

    let mut watcher = Watcher::new(feed_url, interval, scripts);
    watcher.open_database(matches.value_of("database").unwrap())?;
    watcher.probe()?;

    if matches.is_present("init") {
        watcher.process_feed(false)?;
    }

    loop {
        watcher.process_feed(true)?;

        std::thread::sleep(watcher.interval());
    }
}
