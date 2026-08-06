//! Parser for Celeste `.bin` map files.
//!
//! Mirrors `Celeste.BinaryPacker`. The file is a `BinaryWriter` stream:
//!
//! ```text
//! "CELESTE MAP"          dotnet string
//! package name           dotnet string
//! i16                    string-table entry count
//! entry *                dotnet string
//! element                root map element (recursive)
//! ```
//!
//! String values live in a lookup table and every name / string attribute is
//! referenced by a `i16` index. Attribute values carry a `u8` type tag (see
//! [`AttrType`]). Type 7 is a run-length encoded string used by `solids` and
//! `bg` grids.

use crate::data::reader::{ReadError, ReadResult, Reader};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AttrType {
    Bool = 0,
    Byte = 1,
    Short = 2,
    Int = 3,
    Float = 4,
    Str = 5,
    String = 6,
    RleString = 7,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Attr {
    Bool(bool),
    Byte(u8),
    Short(i16),
    Int(i32),
    Float(f32),
    String(String),
    RleString(String),
}

impl Attr {
    #[must_use]
    pub fn as_f32(&self) -> f32 {
        match *self {
            Attr::Bool(b) => u8::from(b) as f32,
            Attr::Byte(b) => b as f32,
            Attr::Short(s) => s as f32,
            Attr::Int(i) => i as f32,
            Attr::Float(f) => f,
            Attr::String(ref s) | Attr::RleString(ref s) => s.parse().unwrap_or(0.0),
        }
    }

    #[must_use]
    pub fn as_bool(&self) -> bool {
        match *self {
            Attr::Bool(b) => b,
            Attr::Byte(b) => b != 0,
            Attr::Short(s) => s != 0,
            Attr::Int(i) => i != 0,
            Attr::Float(f) => f != 0.0,
            Attr::String(ref s) | Attr::RleString(ref s) => matches!(s.as_str(), "true" | "1"),
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Attr::String(s) | Attr::RleString(s) => s,
            _ => "",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, Attr)>,
    pub children: Vec<Element>,
}

impl Element {
    #[must_use]
    pub fn attr(&self, name: &str) -> Option<&Attr> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    #[must_use]
    pub fn attr_f32(&self, name: &str, default: f32) -> f32 {
        self.attr(name).map_or(default, Attr::as_f32)
    }

    #[must_use]
    pub fn attr_bool(&self, name: &str, default: bool) -> bool {
        self.attr(name).map_or(default, Attr::as_bool)
    }

    #[must_use]
    pub fn attr_str(&self, name: &str, default: &str) -> String {
        self.attr(name)
            .map_or_else(|| default.to_string(), |a| a.as_str().to_string())
    }

    #[must_use]
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }

    #[must_use]
    pub fn children_named<'e>(&'e self, name: &'e str) -> impl Iterator<Item = &'e Element> {
        self.children.iter().filter(move |c| c.name == name)
    }
}

#[derive(Debug, Clone)]
pub struct MapBin {
    pub package: String,
    pub root: Element,
}

impl MapBin {
    pub fn from_bytes(bytes: &[u8]) -> ReadResult<MapBin> {
        let mut reader = Reader::new(bytes);
        let magic = reader.read_dotnet_string()?;
        if magic != "CELESTE MAP" {
            return Err(ReadError::new(format!("bad magic: {magic:?}")));
        }
        let package = reader.read_dotnet_string()?.to_string();
        let count = reader.read_i16()?;
        let mut table = Vec::with_capacity(count.max(0) as usize);
        for _ in 0..count {
            table.push(reader.read_dotnet_string()?.to_string());
        }
        let root = read_element(&mut reader, &table)?;
        Ok(MapBin { package, root })
    }

    pub fn from_file(path: &std::path::Path) -> ReadResult<MapBin> {
        let bytes = std::fs::read(path)
            .map_err(|e| ReadError::new(format!("read {}: {e}", path.display())))?;
        Self::from_bytes(&bytes)
    }
}

fn read_element(reader: &mut Reader<'_>, table: &[String]) -> ReadResult<Element> {
    let name = table
        .get(reader.read_i16()? as usize)
        .ok_or_else(|| ReadError::new("name index out of bounds"))?
        .clone();

    let attr_count = reader.read_u8()?;
    let mut attrs = Vec::with_capacity(attr_count as usize);
    for _ in 0..attr_count {
        let key = table
            .get(reader.read_i16()? as usize)
            .ok_or_else(|| ReadError::new("attr name index out of bounds"))?
            .clone();
        let ty = reader.read_u8()?;
        let value = match ty {
            0 => Attr::Bool(reader.read_bool()?),
            1 => Attr::Byte(reader.read_u8()?),
            2 => Attr::Short(reader.read_i16()?),
            3 => Attr::Int(reader.read_i32()?),
            4 => Attr::Float(reader.read_f32()?),
            6 => Attr::String(reader.read_dotnet_string()?.to_string()),
            7 => {
                let len = reader.read_i16()? as usize;
                let bytes = reader.take(len)?;
                Attr::RleString(rle_decode(bytes))
            }
            other => return Err(ReadError::new(format!("unknown attr type {other}"))),
        };
        attrs.push((key, value));
    }

    let child_count = reader.read_i16()?;
    let mut children = Vec::with_capacity(child_count.max(0) as usize);
    for _ in 0..child_count {
        children.push(read_element(reader, table)?);
    }

    Ok(Element {
        name,
        attrs,
        children,
    })
}

fn rle_decode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    let mut i = 0;
    while i + 1 < bytes.len() {
        let count = bytes[i] as usize;
        let ch = bytes[i + 1] as char;
        out.extend(std::iter::repeat(ch).take(count));
        i += 2;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rle_roundtrip() {
        // "aaa" = count 3, char 'a'
        let bytes = [3u8, b'a'];
        assert_eq!(rle_decode(&bytes), "aaa");
    }
}
