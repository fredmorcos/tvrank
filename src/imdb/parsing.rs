use super::error::Err;
use crate::Res;
use atoi::atoi;

pub(crate) const TAB: u8 = b'\t';
pub(crate) const COMMA: u8 = b',';

pub(crate) const TT: &[u8] = b"tt";
pub(crate) const ZERO: &[u8] = b"0";
pub(crate) const ONE: &[u8] = b"1";

pub(crate) const NOT_AVAIL: &[u8] = b"\\N";

pub(crate) fn parse_title_id(id: &[u8]) -> Res<u64> {
  if &id[0..=1] != super::parsing::TT {
    return Err::id();
  }

  Ok(atoi::<u64>(&id[2..]).ok_or(Err::IdNumber)?)
}
