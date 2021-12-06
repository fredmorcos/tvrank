#![warn(clippy::all)]

use tvrank::imdb::{Imdb, ImdbStorage};

fn main() -> tvrank::Res<()> {
  let cache_dir = tempfile::Builder::new().prefix("tvrank_").tempdir()?;
  let storage = ImdbStorage::new(
    cache_dir.path(),
    |db_name, content_len| {
      println!("Starting download of {} (size = {:?})", db_name, content_len);
    },
    |_, _| {},
    |_| {
      println!("Finished download");
    },
    |db_name| {
      println!("Decompressing {}", db_name);
    },
    |_, _| {},
    |_| {
      println!("Finished decompression");
    },
  )?;
  let imdb = Imdb::new(8, &storage)?;

  let name = "city of god";
  let year = Some(2002);

  println!("Matches for {} and {:?}:", name, year);

  for titles in imdb.movies_by_title(name, year)? {
    for title in titles {
      let id = title.title_id();

      println!("ID: {}", id);
      println!("Primary name: {}", title.primary_title());
      println!("Original name: {}", title.original_title());

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
  }

  Ok(())
}
