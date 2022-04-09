#![warn(clippy::all)]

pub(crate) mod tokens {
  /// Returns the next item of an iterator or propagates the end-of-file error if the end of the iterator is reached
  /// # Arguments
  /// * `($iter: ident)` - An iterator
  /// # Errors
  /// Returns an unexpected end-of-file error `Err::Eof` if the end of the file is reached
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

pub(crate) mod io {
  use std::io::Read;

  /// A closure to keep track of the download progress together with the download content
  pub struct Progress<'a, R> {
    inner: R,
    progress_fn: &'a dyn Fn(Option<u64>, u64),
  }

  impl<'a, R: Read> Progress<'a, R> {
    /// Creates a new Progress struct
    /// # Arguments
    /// * `inner` - Download content
    /// * `progress_fn` - Closure to keep track of the download progress
    pub fn new(inner: R, progress_fn: &'a dyn Fn(Option<u64>, u64)) -> Self {
      Self { inner, progress_fn }
    }
  }

  impl<'a, R: Read> Read for Progress<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
      let bytes = self.inner.read(buf)?;
      (self.progress_fn)(None, bytes as u64);
      Ok(bytes)
    }
  }
}
