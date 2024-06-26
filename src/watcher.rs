use rusqlite::types::ToSql;
use tracing::{debug, error};

use std::io::{BufReader, Cursor};
use std::ops::Deref;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::error::FeedParseError;
use crate::{Database, Error};

pub trait FeedExt {
    /// Returns the title of this feed entry, if any.
    fn title(&self) -> Option<&str>;

    /// Returns the link of this feed entry, if any.
    fn link(&self) -> Option<&str>;

    /// Returns the unique id (GUID) of this feed antry, if any.
    fn guid(&self) -> Option<&str>;
}

impl FeedExt for rss::Item {
    fn title(&self) -> Option<&str> {
        self.title()
    }

    fn link(&self) -> Option<&str> {
        self.link()
    }

    fn guid(&self) -> Option<&str> {
        self.guid().map(|guid| guid.value())
    }
}

impl FeedExt for atom_syndication::Entry {
    fn title(&self) -> Option<&str> {
        Some(self.title())
    }

    fn link(&self) -> Option<&str> {
        self.links()
            .iter()
            .find(|link| link.rel() == "alternate")
            .map(|link| link.href())
    }

    fn guid(&self) -> Option<&str> {
        Some(self.id())
    }
}

pub enum Feed {
    Rss(rss::Channel),
    Atom(atom_syndication::Feed),
}

type Url = reqwest::Url;

pub struct Watcher<'a> {
    url: Url,
    executables: Vec<&'a Path>,
    interval: Duration,
    database: Option<Database>,
}

impl<'a> Watcher<'a> {
    pub fn new(url: Url, update_interval: Duration, executables: Vec<&'a Path>) -> Watcher<'a> {
        Watcher {
            url,
            database: None,
            executables,
            interval: update_interval,
        }
    }

    /// Returns the refresh interval.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    pub fn open_database<P: AsRef<Path>>(&mut self, database_path: P) -> Result<(), Error> {
        debug!("Opening database at `{}'", database_path.as_ref().display());

        let mut database = Database::open(database_path)?;
        database.migrate()?;

        self.database = Some(database);

        Ok(())
    }

    fn parse_feed(&self, body: &str) -> Result<Feed, Error> {
        match body.parse::<atom_syndication::Feed>() {
            Ok(feed) => Ok(Feed::Atom(feed)),
            Err(err) => {
                debug!("The response could not be parsed as an atom feed: {}", err);

                match rss::Channel::read_from(BufReader::new(Cursor::new(body))) {
                    Ok(feed) => Ok(Feed::Rss(feed)),
                    Err(err) => Err(Error::FeedParseError(FeedParseError::RssError(err))),
                }
            }
        }
    }

    /// Requests the feed URL and returns the HTTP response body as a string.
    fn request_feed(&self) -> Result<String, Error> {
        let request = reqwest::blocking::get(self.url.as_ref())?;

        request.text().map_err(|e| e.into())
    }

    /// Filters the given slice of entries and returns a Vec with entries that are not currently
    /// saved in the database.
    fn filter_missing_entries(
        &'a self,
        feed_id: i64,
        entries: &'a [&'a dyn FeedExt],
    ) -> Result<Vec<&dyn FeedExt>, Error> {
        let database = self.database.as_ref().unwrap();

        let mut stmt = database
            .connection()
            .prepare("SELECT guid FROM entries WHERE feed_id = ?1 AND guid = ?2")?;

        let new_entries: Vec<&dyn FeedExt> = entries
            .iter()
            .filter(|e| e.guid().is_some())
            .filter(
                |e| match stmt.exists([&feed_id as &dyn ToSql, &e.guid().unwrap()]) {
                    Ok(true) => false,
                    Ok(false) | Err(_) => true,
                },
            )
            .map(Deref::deref)
            .collect::<Vec<&dyn FeedExt>>();

        Ok(new_entries)
    }

    pub fn process_feed(&self, process_entries: bool) -> Result<(), Error> {
        let database = self.database.as_ref().unwrap();

        let feed_id = database
            .get_feed_id_by_url(self.url.as_str())
            .ok_or_else(|| Error::FeedNotFound(self.url.as_str().to_string()))?;

        let body = self.request_feed()?;
        let feed = self.parse_feed(&body);

        let entries: Vec<&dyn FeedExt> = match &feed {
            Ok(Feed::Rss(feed)) => feed.items().iter().map(|i| i as &dyn FeedExt).collect(),
            Ok(Feed::Atom(feed)) => feed.entries().iter().map(|i| i as &dyn FeedExt).collect(),
            Err(_) => vec![],
        };

        let entries = self.filter_missing_entries(feed_id, &entries)?;

        for entry in &entries {
            let guid = entry.guid().unwrap();

            // Exit early since we don't want to execute the scripts - we just want to save them.
            if !process_entries {
                database.try_create_feed_entry(feed_id, guid)?;

                continue;
            }

            for program in &self.executables {
                let status = Command::new(program)
                    .env("FEED_URL", self.url.as_str())
                    .env("FEED_GUID", guid)
                    .env("FEED_LINK", entry.link().unwrap_or(""))
                    .env("FEED_TITLE", entry.title().unwrap_or(""))
                    .status();

                match status {
                    Ok(status) => {
                        if status.success() {
                            debug!("Command `{}' exited successfully", program.display());

                            database.try_create_feed_entry(feed_id, guid)?;
                        } else if let Some(code) = status.code() {
                            error!(
                                "Command `{}' had unexpected exit code: {}",
                                program.display(),
                                code
                            );
                        } else {
                            error!("Command `{}' exited unexpectedly", program.display());
                        }
                    }
                    Err(e) => {
                        error!("Command `{}' failed: {}", program.display(), e);
                    }
                }
            }
            println!("New entry: {:?}", entry.guid());
        }

        Ok(())
    }

    /// Requests the feed url and tries to determine whether it's an RSS or an Atom feed.
    pub fn probe(&mut self) -> Result<(), Error> {
        debug!("Probing feed `{}' to determine type", self.url);

        let body = self.request_feed()?;
        let feed = self.parse_feed(&body);

        let database = self.database.as_ref().expect("database not initialized");

        // Save the feed in the database.
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
}
