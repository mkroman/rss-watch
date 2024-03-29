use std::io;

use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(code(rss_watch::reqwest_error))]
    ReqwestError(#[from] reqwest::Error),

    #[error(transparent)]
    #[diagnostic(code(rss_watch::io_error))]
    IoError(#[from] io::Error),

    #[error(transparent)]
    FeedParseError(#[from] FeedParseError),

    #[error(transparent)]
    #[diagnostic(code(rss_watch::rusqlite_error))]
    RusqliteError(#[from] rusqlite::Error),

    #[error("Feed with URL `{0}' not found in database")]
    FeedNotFound(String),
}

#[derive(Error, Debug, Diagnostic)]
pub enum FeedParseError {
    #[error(transparent)]
    #[diagnostic(code(rss_watch::rss_error))]
    RssError(#[from] rss::Error),
    #[error(transparent)]
    #[diagnostic(code(rss_watch::atom_error))]
    AtomError(#[from] atom_syndication::Error),
}
