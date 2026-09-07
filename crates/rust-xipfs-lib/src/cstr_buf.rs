use core::ffi::CStr;
use core::fmt::{self, Write};

/// CStrBuf is a struct that uses a fixed buffer to avoid needing an allocator.
/// It ties a lifetime `'a` to the buffer and to the result of `add_null_terminator` `&'a CStr` so the
/// CStr only lives as long as the buffer exists.
/// It implements the `write_str` function from the Write trait to allow the use of `write_fmt` and Rust
/// formatting, as well as turn CStrBuf into a writer.
pub struct CStrBuf<'a> {
    buf: &'a mut [u8],
    len: usize,
}

impl<'a> CStrBuf<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, len: 0 }
    }

    pub fn add_null_terminator(self) -> Result<&'a CStr, fmt::Error> {
        if self.len + 1 > self.buf.len() {
            return Err(fmt::Error);
        }
        self.buf[self.len] = 0;
        CStr::from_bytes_with_nul(&self.buf[..=self.len]).map_err(|_| fmt::Error)
    }
}

impl<'a> Write for CStrBuf<'a> {
    /// Take a &str, transform it into bytes and add it to the buffer.
    /// Reserve 1 byte for the trailing NUL needed for a correct CStr.
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        // reserve 1 byte for the trailing NUL, and reject interior NULs
        if self.len + bytes.len() + 1 > self.buf.len() {
            return Err(fmt::Error);
        }
        if bytes.contains(&0) {
            return Err(fmt::Error);
        }
        self.buf[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(())
    }
}
