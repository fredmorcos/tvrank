#![warn(clippy::all)]

//! Helper for showing IO progress using a callback mechanism.

use std::io::Read;

/// An object that offers a callback-based mechanism on `std::io::Read` objects.
///
/// This object maintains a closure of type `F` and the source object (read stream) of
/// type `R`. It forwards `std::io::Read::read` calls to `R` but also calls the closure
/// `F` with how many bytes were read from `R`. This can be used as a progress reporting
/// mechanism.
///
/// `ProgressPipe` reads from a `source` object of type `R` whenever it is read from using
/// its implementation of `std::io::Read`, but instead of immediately forwarding what is
/// read, it first calls the closure of type `F` with the number of bytes that were
/// consumed from the source object.
pub struct ProgressPipe<R, F: Fn(u64)> {
  source: R,
  progress_fn: F,
}

impl<R: Read, F: Fn(u64)> ProgressPipe<R, F> {
  /// Construct a new `Progress` object.
  ///
  /// # Arguments
  ///
  /// * `source` - The source (read stream) object.
  /// * `progress_fn` - The callback closure to keep track of read progress.
  pub fn new(source: R, progress_fn: F) -> Self {
    Self { source, progress_fn }
  }
}

impl<R: Read, F: Fn(u64)> Read for ProgressPipe<R, F> {
  fn read(&mut self, destination: &mut [u8]) -> std::io::Result<usize> {
    let bytes = self.source.read(destination)?;
    (self.progress_fn)(bytes as u64);
    Ok(bytes)
  }
}
