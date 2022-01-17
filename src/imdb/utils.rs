#![warn(clippy::all)]

pub(crate) mod tokens {
  #[macro_export]
  macro_rules! iter_next {
    ($iter:ident) => {{
      $iter.next().ok_or(Err::Eof)?
    }};
  }

  pub(crate) const TAB: u8 = b'\t';
  pub(crate) const COMMA: u8 = b',';

  pub(crate) const TT: &[u8] = b"tt";
  pub(crate) const ZERO: &[u8] = b"0";
  pub(crate) const ONE: &[u8] = b"1";

  pub(crate) const NOT_AVAIL: &[u8] = b"\\N";
}
