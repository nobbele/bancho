use std::time::{Duration, Instant};

#[macro_export]
macro_rules! sum {
    () => (0);
    ($h:expr) => ($h);
    ($h:expr, $($t:expr),*) =>
        ($h + sum!($($t),*));
}

pub trait ByteCount {
    fn byte_count(&self) -> usize;
}

impl ByteCount for String {
    fn byte_count(&self) -> usize {
        1 + 1 + self.len()
    }
}

macro_rules! impl_primitive {
    ($type:ty) => {
        impl ByteCount for $type {
            fn byte_count(&self) -> usize {
                std::mem::size_of::<$type>()
            }
        }
    };
}

impl_primitive!(i64);
impl_primitive!(f32);
impl_primitive!(u32);
impl_primitive!(i32);
impl_primitive!(u16);
impl_primitive!(i16);
impl_primitive!(u8);

pub fn time_diff(a: Instant, b: Instant) -> Duration {
    if a > b {
        a - b
    } else {
        b - a
    }
}

pub fn nonblock_unwrap<T>(res: tokio::io::Result<T>) -> tokio::io::Result<Option<T>> {
    match res {
        Ok(s) => Ok(Some(s)),
        Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => Ok(None),
        Err(e) => Err(e),
    }
}
