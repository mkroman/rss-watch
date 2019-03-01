# rss-watch

[![Build Status](https://travis-ci.org/mkroman/rss-watch.svg?branch=master)](https://travis-ci.org/mkroman/rss-watch)
[![License](https://img.shields.io/badge/License-BSD%202--Clause-orange.svg)](https://opensource.org/licenses/BSD-2-Clause)
![Crates.io](https://img.shields.io/crates/v/rss-watch.svg)

rss-watch is a command-line utillity for watching RSS/Atom feeds and executing a
script whenever there's a new entry on the given feed.

Data about the entry is passed as environment variables to the script.

## Usage

`rss-watch` creates a SQLite database to keep track of previously seen feeds and
entries.

If you don't override the database path using the `-d` flag, the following
default location will be used, depending on your platform.

Platform | Path
---------|-----
*nix     | $XDG_DATA_HOME/rss-watch/database.db
MacOS    | $HOME/Library/Application Support/rss-watch/database.db
Windows  | %LOCALAPPDATA%\mkroman\rss-watch\database.db

It's a good idea to run `rss-watch` with `--init` the first time you're watching
a new feed if you don't want to execute the scripts for all existing entries in
the feed.

```
rss-watch --init -i 6h https://blog.rust-lang.org/feed.xml ./some-script.sh
```

Will start watching the Rust blog and only run `./some-script.sh` when there's a
new entry from this point forward, whereas

```
rss-watch -i 6h https://blog.rust-lang.org/feed.xml ./some-script.sh
```

Will immediately run `./some-script.sh` for each existing entry if this is the
first time this feed is being watched.

## Examples


### Publish a Redis message when there's a new commit

Create the file `publish-redis-message.sh` with the following contents:

```bash
#!/usr/bin/env bash

redis-cli publish some.channel "There's a new commit: ${FEED_LINK}"
```

Make it executable:

`chmod +x ./publish-redis-message.sh`

And run `rss-watch` with

`rss-watch -i 1m https://github.com/mkroman/rss-watch/commits/master.atom
./publish-redis-message.sh`

And it'll now check for new commits once a minute and publish the commit url to
the channel `some.channel`.

## Environment variables

This is a list of the environment variables that can be passed to the script.


Name | Description
-----|-----------
FEED_URL   | The URL to the whole feed. <sup>__Required__</sup>
FEED_GUID  | The unique ID for the given entry. This is used to distinguish the entry from old ones. <sup>__Required__</sup>
FEED_LINK  | Link to the "full story" for this entry. On an Atom entry this will be the first "alternate" link. <sup>_Optional_</sup>
FEED_TITLE | The title of the new entry. <sup>_Optional_</sup>

## Installation

Prerequisites:
* libsqlite3-dev

`rss-watch` is currently only available from crates.io or GitHub.

### Installing the latest stable version with Cargo

```
cargo install rss-watch
```

### Installing from git with Cargo

```
cargo install --git https://github.com/mkroman/rss-watch.git rss-watch
```

## License

This software is licensed under the [BSD 2-clause License](LICENSE).
