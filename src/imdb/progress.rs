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
