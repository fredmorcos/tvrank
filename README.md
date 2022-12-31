# `TVrank`: A Rust library and command-line utility for ranking movies and series

[![License](https://img.shields.io/github/license/fredmorcos/tvrank?style=for-the-badge)](https://github.com/fredmorcos/tvrank/blob/main/LICENSE)
[![Release (latest SemVer)](https://img.shields.io/github/v/release/fredmorcos/tvrank?sort=semver&style=for-the-badge)](https://github.com/fredmorcos/tvrank/releases)
[![Crates.io](https://img.shields.io/crates/v/tvrank?style=for-the-badge)](https://crates.io/crates/tvrank)
[![CI](https://img.shields.io/github/actions/workflow/status/fredmorcos/tvrank/build-test-publish.yml?label=Main&style=for-the-badge)](https://github.com/fredmorcos/tvrank/actions)
</br>
[![docs.rs](https://img.shields.io/docsrs/tvrank?style=for-the-badge)](https://docs.rs/tvrank/0.8.4/tvrank/)
[![Github Open Issues](https://img.shields.io/github/issues-raw/fredmorcos/tvrank?style=for-the-badge)](https://github.com/fredmorcos/tvrank/issues)
[![Github Closed Issues](https://img.shields.io/github/issues-closed-raw/fredmorcos/tvrank?style=for-the-badge)](https://github.com/fredmorcos/tvrank/issues?q=is%3Aissue+is%3Aclosed)
[![Github Open Pull Requests](https://img.shields.io/github/issues-pr-raw/fredmorcos/tvrank?style=for-the-badge)](https://github.com/fredmorcos/tvrank/pulls)
[![Github Closed Pull Requests](https://img.shields.io/github/issues-pr-closed-raw/fredmorcos/tvrank?style=for-the-badge)](https://github.com/fredmorcos/tvrank/pulls?q=is%3Apr+is%3Aclosed)
[![Contributors](https://img.shields.io/github/contributors-anon/fredmorcos/tvrank?style=for-the-badge)](https://github.com/fredmorcos/tvrank/graphs/contributors)

[Github Repository](https://github.com/fredmorcos/tvrank)

`TVrank` is a library and command-line utility written in Rust for querying and ranking
information about movies and series. It can be used to query a single title or scan media
directories.

Currently, `TVrank` only supports IMDB's TSV dumps which it automatically downloads,
caches and periodically updates. More work is required to be able to support and cache
live-query services like [TMDB](https://www.tmdb.org) and [TVDB](https://www.tvdb.org).

The in-memory database is reasonably fast and its on-disk persistent cache format
reasonably efficient.

The library's documentation is badly lacking but there is an example on how to use it.

For now, the command-line utility of `TVrank` works well and fast enough to be usable
e.g. instead of searching for a title through [DuckDuckGo](https://www.duckduckgo.com)
using something like `!imdb TITLE`. In case you still want to see the IMDB page for a
title, `TVrank` will print out a direct link for each search result for direct access from
the terminal.

Note that `TVrank` depends on the `flate2` crate for decompression of IMDB TSV
dumps. `flate2` is extremely slow when built in debug mode, so it is recommended to always
run `TVrank` in release mode unless there are good reasons not to. By default, release
mode is built with debugging information enabled for convenience during development.

## Usage

For information on how to use the library, see below.

The `TVrank` command-line interface has a few modes accessible through the use of
sub-commands:

- `search "KEYWORDS..."` to search by keywords.
- `search "KEYWORDS... (YYYY)"` to search by keywords in a specific year.
- `search "TITLE (YYYY)" --exact` to search for and exact title in a specific year.
- `search "TITLE" --exact` to search for an exact title (`-e` also means exact).
- `scan-movies` and `scan-series` to make batch queries based on directory scans.
- `mark` to mark a directory with a title information file (`tvrank.json`).

### Examples

To search for a specific title:

```sh
$ tvrank search "the great gatsby (2013)" -e
```

To search for all titles containing "the", "great" and "gatsby" in the year 2013:

```sh
$ tvrank search "the great gatsby (2013)"
```

To search based on keywords:

```sh
$ tvrank search "the great gatsby"
```

To search based on an exact title:

```sh
$ tvrank search "the great gatsby" -e
```

To query a series directory:

```sh
$ tvrank scan-series <SERIES_MEDIA_DIR>
```

Also, by default `TVrank` will sort by rating, year and title. To instead sort by year,
rating and title, `--sort-by-year` can be passed before any sub-command:

```sh
$ tvrank --sort-by-year search "house of cards"
```

You can also limit the output of movies and series to the top N entries:

```sh
$ tvrank search "the great gatsby" --top 2
```

You can change the output format to `json` or `yaml`:

```sh
$ tvrank search "the great gatsby" --output json
```

### Batch Queries

`TVrank` can recursively scan directories and print out information about titles it
finds. This is achieved using the `scan-movies` and `scan-series` subcommands.

#### Movie Batch Queries

`TVrank` expects movie directories to be under a top-level movies media directory (herein
called `movies`), as follows:

```
movies
├── ...
├── 127 Hours (2010)
├── 12 Mighty Orphans (2021)
├── 12 Monkeys (1995)
├── 12 Years a Slave (2013)
├── 13 Hours The Secret Soldiers of Benghazi (2016)
├── ...
```

Movie sub-directories are expected to follow the `TITLE (YYYY)` format where the `TITLE`
matches either the primary or original movie title.

If a movie sub-directory does not adhere to this format, `TVrank` will recursively search
it for more titles. An example of that is as follows:

```
movies
├── ...
├── The Naked Gun
│   ├── The Naked Gun (1988)
│   ├── The Naked Gun 2½ The Smell of Fear (1991)
│   └── The Naked Gun 33 1-3 The Final Insult (1994)
├── ...
```

#### Series Batch Queries

`TVrank` also expects series directories to be under a top-level series media directory
(herein called `series`) following either `TITLE` or `TITLE (YYYY)` format. The `TITLE
(YYYY)` format can be used to easily disambiguate similarly-titled series. Examples:

```
series
├── ...
├── House of Cards (1990)
├── Killing Eve
├── Kingdom (2019)
├── ...
```

#### Handling Ambiguity in Batch Queries

Sometimes it is impossible to distinguish between titles just from their original/primary
title and release year, this is due to multiple movies or series being released during the
same year using the same exact title.

To handle this issue, `TVrank` supports the ability to explicitly provide title
information files (called `tvrank.json`) under the corresponding title directory. These
files are detected when using the `scan-movies` and `scan-series` sub-commands and are
used for exact identification using the title's unique ID.

A `tvrank.json` file looks like this:

```json
{
  "imdb": {
    "id": "ttXXXXXXXX"
  }
}
```

where "ttXXXXXXXX" is the IMDB title id shown under the `IMDB ID` column or available as
part of the IMDB URL of a title.

You can ask `TVrank` to write the title information (`tvrank.json`) file for you by using
the `mark` sub-command and passing it the title's directory and ID that you would like to
write.

```sh
tvrank mark "movies/The Great Gatsby (2013)" tt1343092
```

This will results in a file called `movies/The Great Gatsby (2013)/tvrank.json` containing
the following information:

```json
{
  "imdb": {
    "id": "tt1343092"
  }
}
```

If a `tvrank.json` file already exists, `TVrank` will refuse to overwrite it. To force
overwriting it, the `--force` flag can be used.

### Verbosity

To print out more information about what the application is doing, use `-v` before any
sub-command. Multiple occurrences of `-v` on the command-line will increase the verbosity
level:

```sh
$ tvrank -vvv --sort-by-year search "city of god"
```

The following options can come before or after the sub-command. The latter have precedence
over the former.

```sh
--verbose
--sort-by-year
--force-update
--top <N>
--color
--output [table|json|yaml]
```

To find help, see the `help` sub-command:

```sh
$ tvrank help
$ tvrank help search
$ tvrank help scan-series
$ tvrank help scan-movies
```

### Screencast

Please note that the screencast is slightly outdated. Please use the sub-commands
described above instead of what is shown in the screencast.

<p align="center">
    <img src="screencasts/screencast_2021-11-22.gif">
</p>

### Disabling Colors

By default, `TVrank` displays some of the content with color. However, it supports the
`NO_COLOR` environment variable. When `NO_COLOR` is set, `TVrank` will not use color in
its output. This can also be overridden by passing the `--color` argument on the
command-line:

```sh
NO_COLOR=1 tvrank search "the great gatsby"           # Without colors
NO_COLOR=1 tvrank search "the great gatsby" --color   # With colors
```

## Installation

It is recommended to use the [pre-built
releases](https://github.com/fredmorcos/tvrank/releases).

### From source

Installing `TVrank` from this repository's sources requires Cargo, a Rust compiler and a
toolchain to be available. Once those are ready and the repository's contents are cloned,
a simple build and install through cargo should suffice:

```sh
$ git clone https://github.com/fredmorcos/tvrank
$ cd tvrank
$ cargo install --path cli
```

### From Crates.io

Installing `TVrank` from [Crates.io](https://crates.io) also requires Cargo, a Rust
compiler and a toolchain to be available. Once those are ready, a simple build and install
using cargo should suffice:

```sh
$ cargo install tvrank-cli`
```

## Using the library

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
tvrank = "0.8"
```

Or, using `cargo add`:

```sh
$ cargo add tvrank
```

Include the `Imdb` type:

```rust
use tvrank::imdb::{Imdb, ImdbQuery};
use tvrank::utils::search::SearchString;
```

Create a directory for the cache using the `tempfile` crate then create the database
service. The closure passed to the service constructor is a callback for progress updates
and is a `FnMut` to be able to e.g. mutate a progress bar object.

```rust
let cache_dir = tempfile::Builder::new().prefix("tvrank_").tempdir()?;
let imdb = Imdb::new(cache_dir.path(), false, &mut |_| {})?;
```

Afterwards, one can query the database using either `imdb.by_id(...)`,
`imdb.by_title(...)`, `imdb.by_title_and_year(...)` or `imdb.by_keywords(...)`, and print
out some information about the results.

```rust
let title = "city of god";
let year = 2002;

println!("Matches for {} and {:?}:", title, year);

let search_string = SearchString::try_from(title)?;
for title in imdb.by_title_and_year(&search_string, year, ImdbQuery::Movies)? {
  let id = title.title_id();

  println!("ID: {}", id);
  println!("Primary name: {}", title.primary_title());
  if let Some(original_title) = title.original_title() {
    println!("Original name: {}", original_title);
  } else {
    println!("Original name: N/A");
  }

  if let Some((rating, votes)) = title.rating() {
    println!("Rating: {}/100 ({} votes)", rating, votes);
  } else {
    println!("Rating: N/A");
  }

  if let Some(runtime) = title.runtime() {
    println!("Runtime: {}", humantime::format_duration(runtime));
  } else {
    println!("Runtime: N/A");
  }

  println!("Genres: {}", title.genres());
  println!("--");
}
```

See the `query.rs` example under the `lib/examples/query` directory for a
fully-functioning version of the above.
