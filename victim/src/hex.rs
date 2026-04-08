// https://docs.rs/hex/latest/src/hex/lib.rs.html

extern crate alloc;
use alloc::{string::String, vec::Vec};

use core::iter;

const HEX_CHARS_LOWER: &[u8; 16] = b"0123456789abcdef";
pub trait ToHex {
    fn encode_hex<T: iter::FromIterator<char>>(&self) -> T;
}

struct BytesToHexChars<'a> {
    inner: ::core::slice::Iter<'a, u8>,
    table: &'static [u8; 16],
    next: Option<char>,
}

impl<'a> BytesToHexChars<'a> {
    fn new(inner: &'a [u8], table: &'static [u8; 16]) -> BytesToHexChars<'a> {
        BytesToHexChars {
            inner: inner.iter(),
            table,
            next: None,
        }
    }
}

impl<'a> Iterator for BytesToHexChars<'a> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next.take() {
            Some(current) => Some(current),
            None => self.inner.next().map(|byte| {
                let current = self.table[(byte >> 4) as usize] as char;
                self.next = Some(self.table[(byte & 0x0F) as usize] as char);
                current
            }),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let length = self.len();
        (length, Some(length))
    }
}

impl<'a> iter::ExactSizeIterator for BytesToHexChars<'a> {
    fn len(&self) -> usize {
        let mut length = self.inner.len() * 2;
        if self.next.is_some() {
            length += 1;
        }
        length
    }
}

#[inline]
fn encode_to_iter<T: iter::FromIterator<char>>(table: &'static [u8; 16], source: &[u8]) -> T {
    BytesToHexChars::new(source, table).collect()
}

impl<T: AsRef<[u8]>> ToHex for T {
    fn encode_hex<U: iter::FromIterator<char>>(&self) -> U {
        encode_to_iter(HEX_CHARS_LOWER, self.as_ref())
    }
}

#[must_use]
pub fn encode<T: AsRef<[u8]>>(data: T) -> String {
    data.encode_hex()
}