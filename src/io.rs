use crate::Res;
use std::io::{Read, Write};

pub(crate) fn write_interactive<R: Read, W: Write>(
  reader: &mut R,
  writer: &mut W,
  f: impl Fn(usize) -> Res<()>,
) -> Res<usize> {
  let mut buffer: [u8; 4096] = [0; 4096];
  let mut total = 0;

  loop {
    let n = reader.read(&mut buffer)?;
    if n == 0 {
      break;
    }

    total += n;

    let _ = writer.write_all(&buffer[..n])?;

    f(n)?
  }

  Ok(total)
}
