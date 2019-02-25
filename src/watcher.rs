use failure::{Error as FError, Fail};
use log::{debug, error};
use rusqlite::types::ToSql;

use std::io::{BufReader, Cursor};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::error::FeedParseError;
use crate::Error;

const INIT_SQL: &'static str = include_str!("../init.sql");

pub enum FeedKind {
    Rss,
    Atom,
    Undetermined,
}

pub enum Feed {
    Rss(rss::Channel),
    Atom(atom_syndication::Feed),
}

type Url = reqwest::Url;

pub struct Watcher<'a> {
    url: Url,
    executables: Vec<&'a str>,
    pub interval: Duration,
    kind: FeedKind,
    database: Option<Database>,
}

pub struct Database {
    connection: rusqlite::Connection,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Database, Error> {
        let connection = rusqlite::Connection::open(path)?;

        Ok(Database {
            connection: connection,
        })
    }

    pub fn init(&self) -> Result<(), Error> {
        self.connection.execute_batch(INIT_SQL)?;

        Ok(())
    }

    pub fn try_create_feed(&self, feed_url: &str, kind: i64) -> Result<(), Error> {
        self.connection.execute(
            "INSERT OR IGNORE INTO feeds (url, type) VALUES (?1, ?2)",
            &[&feed_url, &kind as &ToSql],
        )?;

        Ok(())
    }

    /// Returns a feed id for a given url if it exists. None otherwise.
    pub fn get_feed_id_by_url(&self, feed_url: &str) -> Option<i64> {
        self.connection
            .query_row(
                "SELECT id FROM feeds WHERE url = ?1",
                &[&feed_url],
                |row| -> i64 { row.get(0) },
            )
            .ok()
    }

    /// Returns a list of GUIDs not already present under the given feed_id.
    pub fn find_missing_guids<'a>(
        &self,
        feed_id: i64,
        guids: &[&'a str],
    ) -> Result<Vec<&'a str>, Error> {
        // TODO: Use virtual tables or figure out how to use carray.

        let mut stmt = self
            .connection
            .prepare("SELECT guid FROM entries WHERE feed_id = ?1 AND guid = ?2")?;

        let mut missing_guids: Vec<&str> = Vec::with_capacity(guids.len());

        for guid in guids.iter() {
            match stmt.exists(&[&feed_id as &ToSql, guid]) {
                Ok(false) => {
                    missing_guids.push(guid);
                }
                Ok(true) => {}
                Err(e) => return Err(e.into()),
            }
        }

        Ok(missing_guids)
    }

    pub fn try_create_feed_entry(&self, feed_id: i64, guid: &str) -> Result<(), Error> {
        self.connection.execute(
            "INSERT INTO entries (feed_id, guid) VALUES (?1, ?2)",
            &[&feed_id as &ToSql, &guid],
        )?;

        Ok(())
    }
}

impl<'a> Watcher<'a> {
    pub fn new(url: &str, update_interval: Duration, executables: Vec<&'a str>) -> Watcher<'a> {
        Watcher {
            url: url.parse().unwrap(),
            database: None,
            executables: executables,
            interval: update_interval,
            kind: FeedKind::Undetermined,
        }
    }

    pub fn open_database<P: AsRef<Path>>(&mut self, database_path: P) -> Result<(), Error> {
        debug!("Opening database at `{}'", database_path.as_ref().display());

        let database = Database::open(database_path)?;
        database.init()?;

        self.database = Some(database);

        Ok(())
    }

    fn parse_feed(&self, body: &str) -> Result<Feed, Error> {
        match body.parse::<atom_syndication::Feed>() {
            Ok(feed) => Ok(Feed::Atom(feed)),
            Err(err) => {
                error!("The response could not be parsed as an atom feed: {}", err);

                match rss::Channel::read_from(BufReader::new(Cursor::new(body))) {
                    Ok(feed) => Ok(Feed::Rss(feed)),
                    Err(e) => Err(Error::FeedParseError {
                        error: FeedParseError::RssError { error: e },
                    }),
                }
            }
        }
    }

    /// Requests the feed URL and returns the HTTP response body as a string.
    fn request_feed(&self) -> Result<String, Error> {
        let mut request = reqwest::get(self.url.as_ref())?;

        request.text().map_err(|e| e.into())
    }

    /// Requests the feed url and tries to determine whether it's an RSS or an Atom feed.
    pub fn probe(&mut self) -> Result<(), Error> {
        debug!("Probing feed `{}' to determine type", self.url);

        let body = self.request_feed()?;
        let feed = self.parse_feed(&body);

        self.kind = match feed {
            Ok(Feed::Rss(_)) => FeedKind::Rss,
            Ok(Feed::Atom(_)) => FeedKind::Atom,
            Err(e) => return Err(e),
        };

        // Save the feed in the database.
        let database = self.database.as_ref().expect("database not initialized");

        match feed {
            Ok(Feed::Rss(_)) => {
                database.try_create_feed(self.url.as_str(), 1 /* 1 = RSS */)?;
            }
            Ok(Feed::Atom(_)) => {
                database.try_create_feed(self.url.as_str(), 2 /* 2 = Atom */)?;
            }
            Err(_) => {}
        }

        Ok(())
    }

    fn process_rss_feed(&self, feed_id: i64, feed: rss::Channel) -> Result<(), Error> {
        let database = self.database.as_ref().unwrap();
        let guids: Vec<&str> = feed
            .items()
            .into_iter()
            .filter_map(|item| item.guid())
            .map(|guid| guid.value())
            .collect();
        let unique_guids = database.find_missing_guids(feed_id, guids.as_ref())?;

        debug!("Saving entries with GUIDs: {:?}", unique_guids);

        let entries: Vec<&rss::Item> = feed
            .items()
            .as_ref()
            .iter()
            .filter(|entry| entry.guid().is_some())
            .filter(|entry| {
                unique_guids
                    .iter()
                    .any(|id| entry.guid().unwrap().value() == *id)
            })
            .collect();

        for entry in entries {
            let guid = entry.guid().map(|guid| guid.value()).unwrap_or("");

            for program in self.executables.iter() {
                let status = Command::new(program)
                    .env("FEED_URL", self.url.as_str())
                    .env("FEED_GUID", guid)
                    .env("FEED_LINK", entry.link().unwrap_or(""))
                    .env("FEED_TITLE", entry.title().unwrap_or(""))
                    .status();

                match status {
                    Ok(status) => {
                        if status.success() {
                            debug!("Command `{}' exited successfully", program);

                            database.try_create_feed_entry(feed_id, guid)?;
                        } else {
                            match status.code() {
                                Some(code) => {
                                    error!(
                                        "Command `{}' had unexpected exit code: {}",
                                        program, code
                                    );
                                }
                                None => error!("Command `{}' exited unexpectedly", program),
                            }
                        }
                    }
                    Err(e) => {
                        error!("Command `{}' failed: {}", program, e);
                    }
                }
            }
        }

        Ok(())
    }

    fn process_atom_feed(&self, feed_id: i64, feed: atom_syndication::Feed) -> Result<(), Error> {
        let database = self.database.as_ref().unwrap();
        let guids: Vec<&str> = feed.entries().into_iter().map(|entry| entry.id()).collect();
        let unique_guids = database.find_missing_guids(feed_id, guids.as_ref())?;

        debug!("Saving entries with GUIDs: {:?}", unique_guids);

        let entries: Vec<&atom_syndication::Entry> = feed
            .entries()
            .as_ref()
            .iter()
            .filter(|entry| unique_guids.iter().any(|id| entry.id() == *id))
            .collect();

        unimplemented!();

        Ok(())
    }

    pub fn process_feed(&self) -> Result<(), Error> {
        let body = self.request_feed()?;
        let feed = self.parse_feed(&body);
        let database = self.database.as_ref().unwrap();

        let feed_id = database
            .get_feed_id_by_url(self.url.as_str())
            .ok_or_else(|| Error::FeedNotFound(self.url.as_str().to_string()))?;

        match feed {
            Ok(Feed::Rss(feed)) => {
                self.process_rss_feed(feed_id, feed)?;
            }
            Ok(Feed::Atom(feed)) => {
                self.process_atom_feed(feed_id, feed)?;
            }
            Err(_) => {
                error!(
                    "Unable to process feed of unknown type: {}",
                    self.url.as_str()
                );
            }
        }

        Ok(())
    }
}
