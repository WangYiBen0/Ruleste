use std::string::String;
use std::vec::Vec;

use crate::types::Vec2;

/// Typed attribute values carried by map entities, mirroring the value types of
/// the original `BinaryPacker` map format.
#[derive(Debug, Clone, PartialEq)]
pub enum MapAttr {
    Bool(bool),
    Byte(u8),
    Short(i16),
    Int(i32),
    Float(f32),
    Str(String),
}

impl core::fmt::Display for MapAttr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MapAttr::Bool(b) => write!(f, "{b}"),
            MapAttr::Byte(b) => write!(f, "{b}"),
            MapAttr::Short(s) => write!(f, "{s}"),
            MapAttr::Int(i) => write!(f, "{i}"),
            MapAttr::Float(v) => write!(f, "{v}"),
            MapAttr::Str(s) => f.write_str(s),
        }
    }
}

impl MapAttr {
    #[must_use]
    pub fn as_f32(&self) -> f32 {
        match *self {
            MapAttr::Bool(b) => u8::from(b) as f32,
            MapAttr::Byte(b) => b as f32,
            MapAttr::Short(s) => s as f32,
            MapAttr::Int(i) => i as f32,
            MapAttr::Float(f) => f,
            MapAttr::Str(ref s) => s.parse().unwrap_or(0.0),
        }
    }

    #[must_use]
    pub fn as_bool(&self) -> bool {
        match *self {
            MapAttr::Bool(b) => b,
            MapAttr::Byte(b) => b != 0,
            MapAttr::Short(s) => s != 0,
            MapAttr::Int(i) => i != 0,
            MapAttr::Float(f) => f != 0.0,
            MapAttr::Str(ref s) => matches!(s.as_str(), "true" | "1"),
        }
    }
}

/// Spawn data handed to a Wasm plugin when an entity is created. Attribute
/// keys and string values are fully resolved (no lookup-table indices).
#[derive(Debug, Clone, Default)]
pub struct MapData {
    pub attrs: Vec<(String, MapAttr)>,
    /// Entity node points (the entity element's child elements, e.g. a wire's
    /// second endpoint). Mirrors `EntityData.Nodes`.
    pub nodes: Vec<Vec2>,
}

impl MapData {
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&MapAttr> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    #[must_use]
    pub fn get_float(&self, key: &str, default: f32) -> f32 {
        self.get(key).map_or(default, MapAttr::as_f32)
    }

    #[must_use]
    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        self.get(key).map_or(default, MapAttr::as_bool)
    }

    #[must_use]
    pub fn get_str(&self, key: &str, default: &str) -> String {
        match self.get(key) {
            Some(MapAttr::Str(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => default.to_string(),
        }
    }

    #[must_use]
    pub fn nodes(&self) -> &[Vec2] {
        &self.nodes
    }

    #[must_use]
    pub fn get_node(&self, index: usize) -> Option<Vec2> {
        self.nodes.get(index).copied()
    }

    /// Serializes this spawn data into a compact binary blob understood by
    /// both the host and the plugins.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.attrs.len() as u32).to_le_bytes());
        for (key, value) in &self.attrs {
            push_str(&mut out, key);
            match value {
                MapAttr::Bool(b) => {
                    out.push(0);
                    out.push(u8::from(*b));
                }
                MapAttr::Byte(b) => {
                    out.push(1);
                    out.push(*b);
                }
                MapAttr::Short(s) => {
                    out.push(2);
                    out.extend_from_slice(&s.to_le_bytes());
                }
                MapAttr::Int(i) => {
                    out.push(3);
                    out.extend_from_slice(&i.to_le_bytes());
                }
                MapAttr::Float(f) => {
                    out.push(4);
                    out.extend_from_slice(&f.to_le_bytes());
                }
                MapAttr::Str(s) => {
                    out.push(5);
                    push_str(&mut out, s);
                }
            }
        }
        out.extend_from_slice(&(self.nodes.len() as u32).to_le_bytes());
        for node in &self.nodes {
            out.extend_from_slice(&node.x.to_le_bytes());
            out.extend_from_slice(&node.y.to_le_bytes());
        }
        out
    }

    /// Parses a blob produced by [`MapData::to_bytes`].
    pub fn from_bytes(bytes: &[u8]) -> Result<MapData, MapError> {
        let mut it = Reader::new(bytes);
        let count = it.u32()? as usize;
        let mut attrs = Vec::with_capacity(count);
        for _ in 0..count {
            let key = it.string()?;
            let tag = it.u8()?;
            let value = match tag {
                0 => MapAttr::Bool(it.u8()? != 0),
                1 => MapAttr::Byte(it.u8()?),
                2 => MapAttr::Short(it.i16()?),
                3 => MapAttr::Int(it.i32()?),
                4 => MapAttr::Float(it.f32()?),
                5 => MapAttr::Str(it.string()?),
                other => return Err(MapError::BadTag(other)),
            };
            attrs.push((key, value));
        }
        let node_count = it.u32()? as usize;
        let mut nodes = Vec::with_capacity(node_count);
        for _ in 0..node_count {
            nodes.push(Vec2::new(it.f32()?, it.f32()?));
        }
        Ok(MapData { attrs, nodes })
    }
}

#[derive(Debug)]
pub enum MapError {
    Truncated,
    BadTag(u8),
}

fn push_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u32).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Reader<'a> {
        Reader { bytes, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], MapError> {
        if self.pos + n > self.bytes.len() {
            return Err(MapError::Truncated);
        }
        let slice = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, MapError> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, MapError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes(b.try_into().unwrap()))
    }

    fn i16(&mut self) -> Result<i16, MapError> {
        let b = self.take(2)?;
        Ok(i16::from_le_bytes(b.try_into().unwrap()))
    }

    fn i32(&mut self) -> Result<i32, MapError> {
        let b = self.take(4)?;
        Ok(i32::from_le_bytes(b.try_into().unwrap()))
    }

    fn f32(&mut self) -> Result<f32, MapError> {
        let b = self.take(4)?;
        Ok(f32::from_le_bytes(b.try_into().unwrap()))
    }

    fn string(&mut self) -> Result<String, MapError> {
        let len = self.u32()? as usize;
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| MapError::Truncated)
    }
}
