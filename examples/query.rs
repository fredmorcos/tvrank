#![warn(clippy::all)]

use tvrank::imdb::{Imdb, ImdbStorage};

fn main() -> tvrank::Res<()> {
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

  let cache_dir = tempfile::Builder::new().prefix("tvrank_").tempdir()?;
  let storage = ImdbStorage::new(
    cache_dir.path(),
    false,
    &(download_init, download_progress, download_finish),
    &(extract_init, extract_progress, extract_finish),
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
