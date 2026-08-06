//! Low-level binary reading helpers shared by all resource parsers.
//!
//! All `.bin` maps and `.meta` atlases produced by the original Celeste use
//! .NET's `BinaryReader`/`BinaryWriter`. Notably, strings are length-prefixed
//! with a 7-bit varint (see `BinaryWriter.Write(string)`), *not* a fixed
//! `u32`. This module mirrors those semantics exactly.

use std::fmt;

#[derive(Debug)]
pub struct ReadError {
    pub msg: String,
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "binary read error: {}", self.msg)
    }
}

impl std::error::Error for ReadError {}

impl ReadError {
    pub fn new(msg: impl Into<String>) -> ReadError {
        ReadError { msg: msg.into() }
    }
}

pub type ReadResult<T> = Result<T, ReadError>;

#[derive(Clone)]
pub struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Reader<'a> {
        Reader { bytes, pos: 0 }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    pub fn is_at_end(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn take(&mut self, n: usize) -> ReadResult<&'a [u8]> {
        if self.remaining() < n {
            return Err(ReadError::new(format!(
                "tried to read {n} bytes but only {} remain",
                self.remaining()
            )));
        }
        let out = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    pub fn read_u8(&mut self) -> ReadResult<u8> {
        Ok(self.take(1)?[0])
    }

    pub fn read_bool(&mut self) -> ReadResult<bool> {
        Ok(self.read_u8()? != 0)
    }

    pub fn read_i16(&mut self) -> ReadResult<i16> {
        let b: [u8; 2] = self.take(2)?.try_into().expect("slice of len 2");
        Ok(i16::from_le_bytes(b))
    }

    pub fn read_i32(&mut self) -> ReadResult<i32> {
        let b: [u8; 4] = self.take(4)?.try_into().expect("slice of len 4");
        Ok(i32::from_le_bytes(b))
    }

    pub fn read_u32(&mut self) -> ReadResult<u32> {
        let b: [u8; 4] = self.take(4)?.try_into().expect("slice of len 4");
        Ok(u32::from_le_bytes(b))
    }

    pub fn read_f32(&mut self) -> ReadResult<f32> {
        Ok(f32::from_bits(self.read_u32()?))
    }

    /// Reads a .NET `BinaryWriter.Write(string)` value: a 7-bit varint length
    /// followed by the UTF-8 bytes.
    pub fn read_dotnet_string(&mut self) -> ReadResult<&'a str> {
        let len = self.read_7bit_varint()?;
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes).map_err(|e| ReadError::new(format!("invalid utf8: {e}")))
    }

    /// Reads a .NET 7-bit encoded length prefix.
    pub fn read_7bit_varint(&mut self) -> ReadResult<usize> {
        let mut shift = 0u32;
        let mut value = 0u32;
        loop {
            let byte = self.read_u8()?;
            value |= u32::from(byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                return Ok(value as usize);
            }
            shift += 7;
            if shift >= 32 {
                return Err(ReadError::new("7-bit varint too long"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotnet_string_roundtrip() {
        // "hello" encoded by .NET BinaryWriter: len=5 (single byte), then bytes.
        let mut bytes = vec![0x05];
        bytes.extend_from_slice(b"hello");
        let mut r = Reader::new(&bytes);
        assert_eq!(r.read_dotnet_string().unwrap(), "hello");
    }

    #[test]
    fn varint_multibyte() {
        // 0x80 encoded as two bytes: 0x80 0x01
        let bytes = [0x80, 0x01];
        let mut r = Reader::new(&bytes);
        assert_eq!(r.read_7bit_varint().unwrap(), 0x80);
    }
}
