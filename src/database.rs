use rusqlite::types::ToSql;

use std::path::Path;

use crate::error::Error;

const INIT_SQL: &'static str = include_str!("../init.sql");

pub struct Database {
    connection: rusqlite::Connection,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Database, Error> {
        let connection = rusqlite::Connection::open(path)?;

        Ok(Database { connection })
    }

    pub fn init(&self) -> Result<(), Error> {
        self.connection.execute_batch(INIT_SQL)?;

        Ok(())
    }

    pub fn connection(&self) -> &rusqlite::Connection {
        &self.connection
    }

    pub fn try_create_feed(&self, feed_url: &str, kind: i64) -> Result<(), Error> {
        self.connection.execute(
            "INSERT OR IGNORE INTO feeds (url, type) VALUES (?1, ?2)",
            &[&feed_url, &kind as &dyn ToSql],
        )?;

        Ok(())
    }

    /// Returns a feed id for a given url if it exists. None otherwise.
    pub fn get_feed_id_by_url(&self, feed_url: &str) -> Option<i64> {
        self.connection
            .query_row("SELECT id FROM feeds WHERE url = ?1", &[&feed_url], |row| {
                row.get(0)
            })
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
            match stmt.exists(&[&feed_id as &dyn ToSql, guid]) {
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
            &[&feed_id as &dyn ToSql, &guid],
        )?;

        Ok(())
    }
}
