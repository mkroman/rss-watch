use log::{debug, error};

use std::io::{BufReader, Cursor};
use std::time::Duration;

use crate::error::FeedParseError;
use crate::Error;

pub enum FeedKind {
    Rss,
    Atom,
    Other,
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
    interval: Duration,
    pub kind: FeedKind,
}

impl<'a> Watcher<'a> {
    pub fn new(url: &str, update_interval: Duration, executables: Vec<&'a str>) -> Watcher<'a> {
        Watcher {
            url: url.parse().unwrap(),
            executables: executables,
            interval: update_interval,
            kind: FeedKind::Undetermined,
        }
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

        request.text().map_err(|e| Error::ReqwestError { error: e })
    }

    /// Requests the feed url and tries to determine whether it's an RSS or an Atom feed.
    pub fn probe(&mut self) -> Result<(), Error> {
        debug!("Probing feed `{}' to determine type", self.url);

        let body = self.request_feed()?;

        self.kind = match self.parse_feed(&body) {
            Ok(Feed::Rss(_)) => FeedKind::Rss,
            Ok(Feed::Atom(_)) => FeedKind::Atom,
            Err(e) => return Err(e),
        };

        Ok(())
    }
}
