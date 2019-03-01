# rss-watch

[![Build Status](https://travis-ci.org/mkroman/rss-watch.svg?branch=master)](https://travis-ci.org/mkroman/rss-watch)
[![License](https://img.shields.io/badge/License-BSD%202--Clause-orange.svg)](https://opensource.org/licenses/BSD-2-Clause)

rss-watch is a command-line utillity for watching RSS/Atom feeds and executing a
script whenever there's a new entry on the given feed.

Data about the entry is passed as environment variables to the script.

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

`rss-watch` is currently only available in source form. Once it becomes more
stable it will be available as an executable.

### Installing with Cargo

Prerequisites:
* libsqlite3-dev

```
cargo install --git https://github.com/mkroman/rss-watch.git rss-watch
```

