# `TVrank`: A Rust library and command-line utility for ranking movies and series

[![License](https://img.shields.io/github/license/fredmorcos/tvrank?style=for-the-badge)](https://github.com/fredmorcos/tvrank/blob/main/LICENSE)
[![Release (latest SemVer)](https://img.shields.io/github/v/release/fredmorcos/tvrank?sort=semver&style=for-the-badge)](https://github.com/fredmorcos/tvrank/releases)
[![Release](https://img.shields.io/github/workflow/status/fredmorcos/tvrank/Release?label=Release&style=for-the-badge)](https://github.com/fredmorcos/tvrank/releases)
[![CI](https://img.shields.io/github/workflow/status/fredmorcos/tvrank/CI?label=Master&style=for-the-badge)](https://github.com/fredmorcos/tvrank/actions)
</br>
[![Crates.io](https://img.shields.io/crates/v/tvrank?style=for-the-badge)](https://crates.io/crates/tvrank)
[![docs.rs](https://img.shields.io/docsrs/tvrank?style=for-the-badge)](https://docs.rs/tvrank/0.7.0/tvrank/)

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

* `title "KEYWORDS..."` to search by keywords.
* `title "KEYWORDS... (YYYY)"` to search by keywords in a specific year.
* `title "TITLE (YYYY)" --exact` to search for and exact title in a specific year.
* `title "TITLE" --exact` to search for an exact title (`-e` also means exact).
* `movies-dir` and `series-dir` to make batch queries based on directory scans.

To search for a specific title:

```sh
$ tvrank title "the great gatsby (2013)" -e
```

To search for all titles containing "the", "great" and "gatsby" in the year 2013:

```sh
$ tvrank title "the great gatsby (2013)"
```

To search based on keywords:

```sh
$ tvrank title "the great gatsby"
```

To search based on an exact title:

```sh
$ tvrank title "the great gatsby" -e
```

To query a series directory:

```sh
$ tvrank series-dir <MEDIA_DIR>
```

Also, by default `TVrank` will sort by rating, year and title. To instead sort by year,
rating and title, `--sort-by-year` can be passed before any sub-command:

```sh
$ tvrank --sort-by-year title "house of cards"
```

You can also limit the output of movies and series to the top N entries:

```sh
$ tvrank title "the great gatsby" --top 2
```

To print out more information about what the application is doing, use `-v` before any
sub-command. Multiple occurrences of `-v` on the command-line will increase the verbosity
level:

```sh
$ tvrank -vvv --sort-by-year title "city of god"
```

The following options can come before or after the sub-command. The latter have precedence
over the former.

```sh
--verbose
--sort-by-year
--force-update
--top
```

To find help, see the `help` sub-command:

```sh
$ tvrank help
$ tvrank help title
$ tvrank help series-dir
$ tvrank help movies-dir
```

### Screencast

Please note that the screencast is slightly outdated. Please use the sub-commands
described above instead of what is shown in the screencast.

<p align="center">
    <img src="screencasts/screencast_2021-11-22.gif">
</p>

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
$ cargo install --profile production --path .
```

### From Crates.io

Installing `TVrank` from [Crates.io](https://crates.io) also requires Cargo, a Rust
compiler and a toolchain to be available. Once those are ready, a simple build and install
using cargo should suffice:

```sh
$ cargo install --profile production tvrank`
```

## Using the library

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
tvrank = "0.7"
```

Or, using `cargo add`:

```sh
$ cargo add tvrank
```

Include the `Imdb` type:

```rust
use tvrank::imdb::{Imdb, ImdbQuery};
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

for title in imdb.by_title_and_year(title, year, ImdbQuery::Movies)? {
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

See the `query` example under the `examples/` directory for a fully-functioning version of
the above.
