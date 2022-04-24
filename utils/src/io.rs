#![warn(clippy::all)]

//! Common utilities for IO.

use std::io::Read;

/// An object that offers a callback-based mechanism on `std::io::Read` objects.
///
/// This object maintains a closure and the object (read stream) `R`. It forwards
/// `std::io::Read::read` calls to `R` but also calls the closure with how many bytes
/// were read from `R`. This can be used as a progress reporting mechanism.
pub struct Progress<'a, R> {
  inner: R,
  progress_fn: &'a dyn Fn(Option<u64>, u64),
}

impl<'a, R: Read> Progress<'a, R> {
  /// Construct a new `Progress` object.
  ///
  /// # Arguments
  ///
  /// * `inner` - The inner (read stream) object.
  /// * `progress_fn` - The callback closure to keep track of read progress.
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
