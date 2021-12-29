#![warn(clippy::all)]

use super::error::Err;
use super::title::TitleId;
use crate::Res;
use atoi::atoi;
use deepsize::DeepSizeOf;
use fnv::FnvHashMap;
use std::borrow::Cow;
use std::str::FromStr;

#[derive(Default, DeepSizeOf)]
pub(crate) struct Ratings {
  ratings: FnvHashMap<usize, (u8, u64)>,
}

impl Ratings {
  pub(crate) fn new_from_buf(buf: &'static [u8]) -> Res<Self> {
    let mut res = Self::default();

    for line in buf.split(|&b| b == b'\n').skip(1) {
      res.add_rating_from_line(line)?;
    }

    Ok(res)
  }

  fn add_rating_from_line(&mut self, line: &'static [u8]) -> Res<()> {
    if line.is_empty() {
      return Ok(());
    }

    let mut iter = line.split(|&b| b == super::parsing::TAB);

    macro_rules! next {
      () => {{
        iter.next().ok_or(Err::Eof)?
      }};
    }

    let title_id = Cow::from(unsafe { std::str::from_utf8_unchecked(next!()) });
    let title_id = TitleId::try_from(title_id)?;
    let rating = f32::from_str(unsafe { std::str::from_utf8_unchecked(next!()) })?;
    let rating = unsafe { (rating * 10.0).to_int_unchecked() };
    let votes = atoi::<u64>(next!()).ok_or(Err::Votes)?;

    if self
      .ratings
      .insert(*AsRef::<usize>::as_ref(&title_id), (rating, votes))
      .is_some()
    {
      return Err::duplicate_id(title_id);
    }

    Ok(())
  }

  pub(crate) fn get<'a>(&'a self, id: &TitleId<'static>) -> Option<&'a (u8, u64)> {
    self.ratings.get(AsRef::<usize>::as_ref(&id))
  }
}
