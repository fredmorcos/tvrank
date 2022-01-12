#![warn(clippy::all)]

use super::error::Err;
use super::title::TitleId;
use crate::Res;
use atoi::atoi;
use fnv::FnvHashMap;
use std::io::BufRead;
use std::str::FromStr;

#[derive(Default)]
pub(crate) struct Ratings {
  ratings: FnvHashMap<usize, (u8, u64)>,
}

impl Ratings {
  pub(crate) fn new_from_reader<R: BufRead>(mut reader: R) -> Res<Self> {
    let mut res = Self::default();
    let mut line = String::new();

    // Skip the first line.
    let _ = reader.read_line(&mut line)?;
    line.clear();

    loop {
      let bytes = reader.read_line(&mut line)?;

      if bytes == 0 {
        break;
      }

      res.add_rating_from_line(line.as_ref())?;
      line.clear();
    }

    Ok(res)
  }

  pub(crate) fn new_from_buf(buf: &[u8]) -> Res<Self> {
    let mut res = Self::default();

    for line in buf.split(|&b| b == b'\n').skip(1) {
      res.add_rating_from_line(line)?;
    }

    Ok(res)
  }

  fn add_rating_from_line(&mut self, line: &[u8]) -> Res<()> {
    if line.is_empty() {
      return Ok(());
    }

    let mut iter = line.split(|&b| b == super::parsing::TAB);

    macro_rules! next {
      () => {{
        iter.next().ok_or(Err::Eof)?
      }};
    }

    let id = TitleId::try_from(next!())?;
    let rating = f32::from_str(unsafe { std::str::from_utf8_unchecked(next!()) })?;
    let rating = unsafe { (rating * 10.0).to_int_unchecked() };
    let votes = atoi::<u64>(next!()).ok_or(Err::Votes)?;

    if self.ratings.insert(id.as_usize(), (rating, votes)).is_some() {
      return Err::duplicate_id(id.as_str().to_owned());
    }

    Ok(())
  }

  pub(crate) fn get<'a>(&'a self, id: &TitleId<'static>) -> Option<&'a (u8, u64)> {
    self.ratings.get(&id.as_usize())
  }

  pub(crate) fn len(&self) -> usize {
    self.ratings.len()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use indoc::indoc;

  fn make_ratings_reader() -> impl BufRead {
    indoc! {"
      tconst	averageRating	numVotes
      tt0000001	5.7	1845
      tt0000002	6.0	236
      tt0000003	6.5	1603
      tt0000004	6.0	153
      tt0000005	6.2	2424
      tt0000006	5.2	158
      tt0000007	5.4	758
      tt0000008	5.5	1988
      tt0000009	5.9	191
      tt0000010	6.9	6636
    "}
    .as_bytes()
  }

  #[test]
  fn test_ratings_csv() {
    let reader = make_ratings_reader();
    let ratings = Ratings::new_from_reader(reader).unwrap();
    assert_eq!(ratings.len(), 10);

    let id = TitleId::try_from("tt0000001".as_bytes()).unwrap();
    assert_eq!(ratings.get(&id), Some(&(57, 1845)));

    let id = TitleId::try_from("tt0000010".as_bytes()).unwrap();
    assert_eq!(ratings.get(&id), Some(&(69, 6636)));

    let id = TitleId::try_from("tt0000011".as_bytes()).unwrap();
    assert_eq!(ratings.get(&id), None);
  }
}
