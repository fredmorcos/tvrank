# `TVrank`: A Rust library and command-line utility for ranking movies and series

[![License](https://img.shields.io/github/license/fredmorcos/tvrank?style=for-the-badge)](https://github.com/fredmorcos/tvrank/blob/main/LICENSE)
[![Release (latest SemVer)](https://img.shields.io/github/v/release/fredmorcos/tvrank?sort=semver&style=for-the-badge)](https://github.com/fredmorcos/tvrank/releases)
[![Release](https://img.shields.io/github/workflow/status/fredmorcos/tvrank/Release?label=Release&style=for-the-badge)](https://github.com/fredmorcos/tvrank/releases)
[![CI](https://img.shields.io/github/workflow/status/fredmorcos/tvrank/CI?label=Master&style=for-the-badge)](https://github.com/fredmorcos/tvrank/actions)
</br>
[![Crates.io](https://img.shields.io/crates/v/tvrank?style=for-the-badge)](https://crates.io/crates/tvrank)
[![docs.rs](https://img.shields.io/docsrs/tvrank?style=for-the-badge)](https://docs.rs/tvrank/0.3.0/tvrank/)

[Github Repository](https://github.com/fredmorcos/tvrank)

`tvrank` is a library and command-line utility written in Rust for querying and ranking
information about movies and series. It can be used to query a single title or scan
directories.

Currently, `tvrank` only supports IMDB's TSV dumps which it automatically downloads,
caches and periodically updates. More work is required to be able to support and cache
live-query services like TMDB.

Additionally, the "in-memory database" could use improvements through indexing and through
adding support for a persistent cache. Also, the library's documentation is missing but
there is an example on how to use it.

For now, the command-line utility of `tvrank` works well and fast enough to be usable.

Note that `tvrank` depends on the `flate2` crate for decompression of IMDB TSV
dumps. `flate2` is extremely slow when built in debug mode, so it is recommended to always
run `tvrank` in release mode unless there are good reasons not to. By default, release
mode is built with debugging information enabled for convenience during development.

## Usage

For information on how to use the library, see below.

The `tvrank` command-line interface has a few modes enabled by the use of sub-commands:
`title "TITLE (YYYY)"` to search for titles (by title and year), `title "keyword1 keyword2
..."` to search titles based on keywords, `movies-dir` and `series-dir` to make batch
queries based on directory scans.

To query a single title:

```sh
tvrank title "the great gatsby (2013)"
```

To query based on keywords:

```sh
tvrank title "the great gatsby"
```

To query a series directory:

```sh
tvrank series-dir <MEDIA_DIR>
```

Also, by default `tvrank` will sort by rating, year and title. To instead sort by year,
rating and title, `--sort-by-year` can be passed before any sub-command:

```sh
tvrank --sort-by-year title "house of cards"
```

To print out more information about what the application is doing, use `-v` before any
sub-command. Multiple occurrences of `-v` on the command-line will increase the verbosity
level:

```sh
tvrank -vvv --sort-by-year title "city of god"
```

To find help, see the `help` sub-command:

```sh
tvrank help
tvrank help title
tvrank help series-dir
tvrank help movies-dir
```

### Screencast

Please note that the screencast is slightly outdated. Please use the `movies-dir` or
`series-dir` sub-commands instead of `-d` as used in the screencast.

<p align="center">
    <img src="screencasts/screencast_2021-11-22.gif">
</p>

## Using the library

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
tvrank = "0.3"
```

Or, using `cargo add`:

```sh
$ cargo add tvrank
```

Include the `Imdb` type:

```rust
use tvrank::imdb::Imdb;
```

Create a directory for the cache using the `tempfile` crate, create some functions that
will work as callbacks for status updates by the storage backend, and pass all of that to
the `ImdbStorage` constructor then create the database service:

```rust
let cache_dir = tempfile::Builder::new().prefix("tvrank_").tempdir()?;

fn download_init(name: &str, content_len: Option<u64>) {
  println!("Starting download of {} (size = {:?})", name, content_len);
}

fn download_progress(_userdata: &(), _delta: u64) {}

fn download_finish(_userdata: &()) {
  println!("Finished download");
}

fn extract_init(name: &str) {
  println!("Extracting {}", name);
}

fn extract_progress(_userdata: &(), _delta: u64) {}

fn extract_finish(_userdata: &()) {
  println!("Finished extracting");
}

let storage = ImdbStorage::new(
  cache_dir.path(),
  false,
  &(download_init, download_progress, download_finish),
  &(extract_init, extract_progress, extract_finish),
)?;
let imdb = Imdb::new(8, &storage)?;
```

Note that the storage constructor takes 6 closures as callbacks: `download_init`,
`download_progress`, `download_finish`, `extract_init`, `extract_progress` and
`extract_finish`. These are meant to print progress, or create a progress bar object and
pass it around for updates.

Afterwards, one can query the database using either `imdb.by_id(...)`,
`imdb.by_title_and_year(...)` or `imdb.by_keywords(...)`, and print out some information
about the results.

```rust
let title = "city of god";
let year = 2002;

println!("Matches for {} and {:?}:", title, year);

for title in imdb.by_title_and_year(title, year, ImdbQueryType::Movies)? {
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

See the `examples/` directory for a fully-functioning version of the above.
