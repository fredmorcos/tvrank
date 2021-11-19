#![warn(clippy::all)]

use std::{collections::HashMap, mem::size_of, rc::Rc, sync::Arc};

pub trait MemSize {
  fn mem_size(&self) -> usize;
}

macro_rules! impl_memsize {
  ($ty:ty) => {
    impl MemSize for $ty {
      fn mem_size(&self) -> usize {
        size_of::<$ty>()
      }
    }
  };
}

impl_memsize!(());
impl_memsize!(bool);
impl_memsize!(u8);
impl_memsize!(u16);
impl_memsize!(u32);
impl_memsize!(u64);
impl_memsize!(u128);
impl_memsize!(i8);
impl_memsize!(i16);
impl_memsize!(i32);
impl_memsize!(i64);
impl_memsize!(i128);
impl_memsize!(f32);
impl_memsize!(f64);
impl_memsize!(char);
impl_memsize!(usize);
impl_memsize!(isize);

impl<T: MemSize> MemSize for Rc<T> {
  fn mem_size(&self) -> usize {
    let val: &T = self;
    size_of::<Self>() + val.mem_size()
  }
}

impl<T: MemSize> MemSize for Arc<T> {
  fn mem_size(&self) -> usize {
    let val: &T = self;
    size_of::<Self>() + val.mem_size()
  }
}

impl MemSize for Arc<str> {
  fn mem_size(&self) -> usize {
    size_of::<usize>() + (self.as_bytes().len() * size_of::<u8>())
  }
}

impl MemSize for Arc<[u8]> {
  fn mem_size(&self) -> usize {
    size_of::<usize>() + (self.len() * size_of::<u8>())
  }
}

impl<T: MemSize> MemSize for Vec<T> {
  fn mem_size(&self) -> usize {
    let elements: usize = self.iter().map(|e| e.mem_size()).sum();
    size_of::<Self>() + elements
  }
}

impl<K: MemSize, V: MemSize, H> MemSize for HashMap<K, V, H> {
  fn mem_size(&self) -> usize {
    let elements: usize = self.iter().map(|(k, v)| k.mem_size() + v.mem_size()).sum();
    size_of::<Self>() + elements
  }
}

impl MemSize for String {
  fn mem_size(&self) -> usize {
    size_of::<Self>() + (self.as_bytes().len() * size_of::<u8>())
  }
}

impl<T: MemSize> MemSize for Option<T> {
  fn mem_size(&self) -> usize {
    let stack = size_of::<Self>();

    match self {
      Some(e) => stack + e.mem_size(),
      None => stack,
    }
  }
}

impl<A: MemSize> MemSize for (A,) {
  fn mem_size(&self) -> usize {
    size_of::<Self>() + self.0.mem_size()
  }
}

impl<A: MemSize, B: MemSize> MemSize for (A, B) {
  fn mem_size(&self) -> usize {
    size_of::<Self>() + self.0.mem_size() + self.1.mem_size()
  }
}

impl<A: MemSize, B: MemSize, C: MemSize> MemSize for (A, B, C) {
  fn mem_size(&self) -> usize {
    size_of::<Self>() + self.0.mem_size() + self.1.mem_size() + self.2.mem_size()
  }
}
