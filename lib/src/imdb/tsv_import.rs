#![warn(clippy::all)]

use std::io::{BufRead, Write};

use crate::imdb::ratings::Ratings;
use crate::imdb::title::Title;
use crate::imdb::title::TsvAction;
use crate::utils::result::Res;

/// Import title data from tab separated values (TSVs).
///
/// This parses TSV data from the provided `ratings_reader` and `basics_reader` and
/// write them out in binary to the provided writers `movies_db_writer` and
/// `series_db_writer`.
///
/// # Arguments
///
/// * `ratings_reader` - TSV reader for ratings.
/// * `basics_reader` - TSV reader for title data.
/// * `movies_db_writer` - Binary writer to store movies.
/// * `series_db_writer` - Binary writer to store series.
pub(crate) fn tsv_import<R1: BufRead, R2: BufRead, W1: Write, W2: Write>(
  ratings_reader: R1,
  mut basics_reader: R2,
  mut movies_db_writer: W1,
  mut series_db_writer: W2,
) -> Res<()> {
  let ratings = Ratings::from_tsv(ratings_reader)?;

  let mut line = String::new();

  // Skip the first line.
  basics_reader.read_line(&mut line)?;
  line.clear();

  loop {
    let bytes = basics_reader.read_line(&mut line)?;

    if bytes == 0 {
      break;
    }

    let trimmed = line.trim_end();

    if trimmed.is_empty() {
      continue;
    }

    match Title::from_tsv(trimmed.as_bytes(), &ratings)? {
      TsvAction::Movie(title) => title.write_binary(&mut movies_db_writer)?,
      TsvAction::Series(title) => title.write_binary(&mut series_db_writer)?,
      TsvAction::Skip => {
        line.clear();
        continue;
      }
    }

    line.clear();
  }

  Ok(())
}
