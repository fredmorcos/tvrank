#![warn(clippy::all)]

//! Common utilities for IO.

use std::io::Read;

/// An object that offers a callback-based mechanism on `std::io::Read` objects.
///
/// This object maintains a closure of type `F` and the object (read stream) of type
/// `R`. It forwards `std::io::Read::read` calls to `R` but also calls the closure `F`
/// with how many bytes were read from `R`. This can be used as a progress reporting
/// mechanism.
pub struct Progress<R, F: Fn(Option<u64>, u64)> {
  inner: R,
  progress_fn: F,
}

impl<R: Read, F: Fn(Option<u64>, u64)> Progress<R, F> {
  /// Construct a new `Progress` object.
  ///
  /// # Arguments
  ///
  /// * `inner` - The inner (read stream) object.
  /// * `progress_fn` - The callback closure to keep track of read progress.
  pub fn new(inner: R, progress_fn: F) -> Self {
    Self { inner, progress_fn }
  }
}

impl<R: Read, F: Fn(Option<u64>, u64)> Read for Progress<R, F> {
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    let bytes = self.inner.read(buf)?;
    (self.progress_fn)(None, bytes as u64);
    Ok(bytes)
  }
}
