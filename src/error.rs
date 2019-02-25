use failure::Fail;
use reqwest;

#[derive(Fail, Debug)]
pub enum Error {
    #[fail(display = "A network error occurred: {}", error)]
    ReqwestError {
        #[fail(cause)]
        error: reqwest::Error,
    },
    #[fail(display = "Unable to parse feed: {}", error)]
    FeedParseError {
        #[fail(cause)]
        error: FeedParseError,
    },
    #[fail(display = "Database error: {}", error)]
    RusqliteError {
        #[fail(cause)]
        error: rusqlite::Error,
    },

    #[fail(
        display = "Feed with the URL `{}' could not be found in the database",
        _0
    )]
    FeedNotFound(String),
    #[fail(display = "IO error: {}", error)]
    IoError { error: std::io::Error },
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Error::ReqwestError { error: error }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::IoError { error: error }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Error::RusqliteError { error: error }
    }
}

#[derive(Fail, Debug)]
pub enum FeedParseError {
    #[fail(display = "RSS parsing error")]
    RssError {
        #[fail(cause)]
        error: rss::Error,
    },
    #[fail(display = "Atom parsing error")]
    AtomError {
        #[fail(cause)]
        error: atom_syndication::Error,
    },
}
