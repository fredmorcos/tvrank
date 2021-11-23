#![warn(clippy::all)]

use tvrank::imdb::Imdb;

fn main() -> tvrank::Res<()> {
  let cache_dir = tempfile::Builder::new().prefix("tvrank_").tempdir()?;
  let imdb = Imdb::new(cache_dir.path())?;

  let name = "city of god";
  let year = Some(2002);

  println!("Matches for {} and {:?}:", name, year);

  for result in imdb.movies(name.as_bytes(), year)? {
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

    println!("--");
  }

  Ok(())
}
