#![warn(clippy::all)]

use tvrank::imdb::{Imdb, ImdbQueryType};

fn main() -> tvrank::Res<()> {
  let cache_dir = tempfile::Builder::new().prefix("tvrank_").tempdir()?;
  let imdb = Imdb::new(cache_dir.path(), false, &mut |_| {})?;

  let title = "city of god";
  let year = 2002;

  println!("Matches for {} and {:?}:", title, year);

  for title in imdb.by_title_and_year(title, year, ImdbQueryType::Movies) {
    let id = title.title_id();

    println!("ID: {}", id);
    println!("Primary name: {}", title.primary_title());
    if let Some(original_title) = title.original_title() {
      println!("Original name: {}", original_title);
    } else {
      println!("Original name: N/A");
    }

    if let Some(rating) = title.rating() {
      println!("Rating: {}/100 ({} votes)", rating.rating(), rating.votes());
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

  Ok(())
}
