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
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Error::ReqwestError { error: error }
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
