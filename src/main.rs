use app_dirs::{get_app_dir, AppDataType, AppInfo};
use clap::{crate_authors, crate_version, App, Arg};
use failure::Fail;
use log::{debug, info};

use std::path::Path;
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

mod error;
mod watcher;
use watcher::Watcher;

pub use error::Error;

const APP_INFO: AppInfo = AppInfo {
    name: "rss-watch",
    author: "Mikkel Kroman",
};

#[derive(Debug, Fail)]
enum CliError {
    #[fail(display = "{} is not a valid interval", _0)]
    InvalidInterval(String),
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
        CliError::WatcherError { error: error }
    }
}

#[cfg(unix)]
fn is_executable<P: AsRef<Path>>(path: P) -> bool {
    let metadata = match path.as_ref().metadata() {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };

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
            eprintln!("Error: {} - {:?}", e, e);
        }
    }
}

fn try_main() -> Result<(), CliError> {
    // Initialize logging.
    env_logger::init();

    let default_database_path =
        get_app_dir(AppDataType::UserData, &APP_INFO, "database.db").unwrap();

    let matches = App::new("rss-watch")
        .version(crate_version!())
        .author(crate_authors!("\n"))
        .about("Scriptable RSS/Atom feed watching tool")
        .arg(
            Arg::with_name("exec")
                .short("e")
                .long("exec")
                .value_name("BIN")
                .takes_value(true)
                .required(true)
                .multiple(true)
                .number_of_values(1),
        )
        .arg(
            Arg::with_name("interval")
                .short("i")
                .long("interval")
                .help("Feed refresh interval in seconds")
                .value_name("INTERVAL")
                .default_value("3600")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("database")
                .short("d")
                .long("database")
                .value_name("DATABASE")
                .help("Database file path")
                .default_value(default_database_path.to_str().unwrap())
                .required(true),
        )
        .arg(Arg::with_name("url").required(true))
        .get_matches();

    let feed_url = matches.value_of("url").unwrap();
    let executables: Vec<&str> = matches.values_of("exec").unwrap_or_default().collect();

    for path in executables.iter().filter(|e| !is_executable(e)) {
        return Err(CliError::ScriptNotExecutable(path.to_string()));
    }

    debug!("Feed URL: {}", feed_url);

    let interval = matches
        .value_of("interval")
        .ok_or(CliError::MissingInterval)
        .and_then(|interval| {
            interval
                .parse()
                .map_err(|_| CliError::InvalidInterval(interval.to_string()))
        })
        .map(|i| Duration::from_secs(i))?;

    debug!("Update interval: {:?}", interval);

    let mut watcher = Watcher::new(feed_url.into(), interval, executables);
    watcher.open_database(matches.value_of("database").unwrap())?;
    watcher.probe()?;

    loop {
        watcher.process_feed()?;

        std::thread::sleep(watcher.interval);
    }
}
