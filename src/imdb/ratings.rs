#![warn(clippy::all)]

use super::error::Err;
use super::title::TitleId;
use crate::Res;
use atoi::atoi;
use deepsize::DeepSizeOf;
use fnv::FnvHashMap;
use std::str::FromStr;

#[derive(Default, DeepSizeOf)]
pub(crate) struct Ratings {
  ratings: FnvHashMap<TitleId, (u8, u64)>,
}

impl Ratings {
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

    let title_id = TitleId::from(super::parsing::parse_title_id(next!())?);
    let rating = f32::from_str(unsafe { std::str::from_utf8_unchecked(next!()) })?;
    let rating = unsafe { (rating * 10.0).to_int_unchecked() };
    let votes = atoi::<u64>(next!()).ok_or(Err::Votes)?;

    if self.ratings.insert(title_id, (rating, votes)).is_some() {
      return Err::duplicate(title_id);
    }

    Ok(())
  }

  pub(crate) fn get(&self, title_id: TitleId) -> Option<&(u8, u64)> {
    self.ratings.get(&title_id)
  }
}
