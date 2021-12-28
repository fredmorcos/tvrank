#![warn(clippy::all)]

use std::error::Error;

pub type Res<T> = Result<T, Box<dyn Error>>;

pub mod imdb;

mod io {
  use crate::Res;
  use std::io::{Read, Write};

  pub(crate) fn write_interactive<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    f: impl Fn(u64),
  ) -> Res<usize> {
    let mut buffer: [u8; 1024 * 16] = [0; 1024 * 16];
    let mut total = 0;

    loop {
      let n = reader.read(&mut buffer)?;
      if n == 0 {
        break;
      }

      total += n;

      let _ = writer.write_all(&buffer[..n])?;

      f(n.try_into()?)
    }

    Ok(total)
  }
}
