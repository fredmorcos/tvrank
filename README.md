# `TVrank`: A Rust library and command-line utility for ranking movies and series

[![License](https://img.shields.io/github/license/fredmorcos/tvrank?style=for-the-badge)](https://github.com/fredmorcos/tvrank/blob/main/LICENSE)
[![Crates.io](https://img.shields.io/crates/v/tvrank?style=for-the-badge)](https://crates.io/crates/tvrank)
[![docs.rs](https://img.shields.io/docsrs/tvrank?style=for-the-badge)](https://docs.rs/tvrank/0.2.3/tvrank/)

`tvrank` is a library and command-line utility written in Rust for querying and ranking
information about movies and series. It can be used to query a single title or scan
directories.

Currently, `tvrank` only supports IMDB's TSV dumps which it automatically downloads,
caches and periodically updates. More work is required to be able to support and cache
live-query services like TMDB.

Additionally, the in-memory database can be vastly improved through indexing. Also, the
library's documentation is missing but there is an example on how to use it.

For now, the command-line utility of `tvrank` works well and fast enough to be usable.

Note that `tvrank` depends on the `flate2` crate for decompression of IMDB TSV
dumps. `flate2` is extremely slow when built in debug mode, so it is recommended to always
run `tvrank` in release mode unless there are good reasons not to. By default, release
mode is built with debugging information enabled for convenience during development.

## Usage

For information on how to use the library, see below.

The `tvrank` command-line interface has two modes modeled as sub-commands: `title` to
query a single title given on the command-line, and `dir` to make batch queries based on
directory scans.

To query a single title:

```sh
tvrank title "the great gatsby"
```

or

```sh
tvrank title "the great gatsby (2013)"
```

To query a directory:

```sh
tvrank dir <MEDIA_DIR>
```

By default `tvrank` will query the movies database, to instead query the series database,
`--series` can be passed before any sub-command:

```sh
tvrank --series dir <MEDIA_DIR>
```

Also, by default `tvrank` will sort by year, rating and title. To instead sort by rating,
year and title, `--sort-by-rating` can be passed before any sub-command:

```sh
tvrank --sort-by-rating --series title "house of cards"
```

To print out more information about what the application is doing, use `-v` before any
sub-command. Multiple occurrences of `-v` on the command-line will increase the verbosity
level:

```sh
tvrank -vvv --sort-by-rating "city of god"
```

To find help, see the `help` sub-command:

```sh
tvrank help
tvrank help title
tvrank help dir
```

### Screencast

Please note that the screencast is slightly outdated. Please use the `dir` sub-command
instead of `-d` as used in the screencast.

<p align="center">
    <img src="screencasts/screencast_2021-11-22.gif">
</p>

## Using the library

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
tvrank = "0.1"
```

Or, using `cargo add`:

```sh
$ cargo add tvrank
```

Include the `Imdb` type:

```rust
use tvrank::imdb::Imdb;
```

Create a directory for the cache using the `tempfile` crate, and pass that to the `Imdb`
database constructor:

```rust
  let cache_dir = tempfile::Builder::new().prefix("tvrank_").tempdir()?;
  let imdb = Imdb::new(cache_dir.path())?;
```

Then, query the database using either `imdb.movies(...)` or `imdb.series(...)`, and print
out some information about the results. Note the use of `imdb.movies_names(...)` or
`imdb.series_names(...)` to query the available titles for a result and the use of
`imdb.rating(...)` to query the rating of a result:

```rust
  let results = imdb.movies("city of god".as_bytes(), Some(2002))?;

  for result in results {
    let id = result.title_id();

    println!("ID: {}", id);

    for name in imdb.movies_names(id)? {
      println!("Name: {}", name);
    }

    if let Some((rating, votes)) = imdb.rating(id) {
      println!("Rating: {}/100 ({} votes)", rating, votes);
    } else {
      println!("Rating: N/A");
    }

    if let Some(runtime) = result.runtime() {
      println!("Runtime: {}", humantime::format_duration(runtime));
    } else {
      println!("Runtime: N/A");
    }

    println!("Genres: {}", result.genres());
  }
```

See the `examples/` directory for a fully-functioning version of the above.
