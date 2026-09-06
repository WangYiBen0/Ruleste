//! Atlas parser and RLE texture decoder for the original `Content/Graphics/Atlases`.
//!
//! A `.meta` file (format `Packer` in `Monocle.Atlas`) lists texture pages and
//! the frames packed into each page:
//!
//! ```text
//! i32              unused (written by the packer)
//! dotnet string    source dir (e.g. "Graphics/SOURCES/...")
//! i32              unused
//! i16              page count
//! page *:
//!   dotnet string  page name (e.g. "Gameplay0")
//!   i16            frame count
//!   frame *:
//!     dotnet string  frame id (backslashes become slashes)
//!     i16 i16 i16 i16   clip x, y, w, h
//!     i16 i16 i16 i16   frame offset x, y, frame w, h
//! optional "LINKS" trailer
//! ```
//!
//! The `.data` page texture is a run-length-encoded RGBA stream (see
//! `Monocle.VirtualTexture`):
//!
//! ```text
//! u32 width
//! u32 height
//! u8  has-alpha flag
//! runs of solid color:
//!   u8 run-len (pixels, in multiples of 4)
//!   if has-alpha:
//!     if next u8 alpha > 0: u8 b, g, r, alpha   (5 bytes total w/ len)
//!     else:                 (transparent run, 2 bytes total)
//!   else:
//!     u8 b, g, r          (4 bytes total w/ len, alpha=255)
//! ```

use std::collections::HashMap;
use std::path::Path;

use crate::data::reader::{ReadError, ReadResult, Reader};

#[derive(Debug, Clone, Copy)]
pub struct FrameRect {
    pub x: i16,
    pub y: i16,
    pub w: i16,
    pub h: i16,
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub id: String,
    pub clip: FrameRect,
    pub offset: FrameRect,
}

#[derive(Debug, Clone)]
pub struct AtlasMeta {
    pub source_dir: String,
    pub pages: Vec<Page>,
    /// Alias mapping from the optional "LINKS" trailer: links[name] = real_id.
    pub links: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Page {
    pub name: String,
    pub frames: Vec<Frame>,
}

impl AtlasMeta {
    pub fn from_bytes(bytes: &[u8]) -> ReadResult<AtlasMeta> {
        let mut r = Reader::new(bytes);
        r.read_i32()?; // unused
        let source_dir = r.read_dotnet_string()?.to_string();
        r.read_i32()?; // unused
        let page_count = r.read_i16()?;
        let mut pages = Vec::with_capacity(page_count.max(0) as usize);
        for _ in 0..page_count {
            let name = r.read_dotnet_string()?.to_string();
            let frame_count = r.read_i16()?;
            let mut frames = Vec::with_capacity(frame_count.max(0) as usize);
            for _ in 0..frame_count {
                let id = r.read_dotnet_string()?.replace('\\', "/");
                let clip = FrameRect {
                    x: r.read_i16()?,
                    y: r.read_i16()?,
                    w: r.read_i16()?,
                    h: r.read_i16()?,
                };
                let offset = FrameRect {
                    x: r.read_i16()?,
                    y: r.read_i16()?,
                    w: r.read_i16()?,
                    h: r.read_i16()?,
                };
                frames.push(Frame { id, clip, offset });
            }
            pages.push(Page { name, frames });
        }

        // Parse optional "LINKS" trailer (alias mapping).
        let mut links = HashMap::new();
        if !r.is_at_end() {
            let marker = r.read_dotnet_string()?;
            if marker == "LINKS" {
                let count = r.read_i16()?;
                for _ in 0..count {
                    let key = r.read_dotnet_string()?.to_string();
                    let value = r.read_dotnet_string()?.to_string();
                    links.insert(key, value);
                }
            }
        }

        Ok(AtlasMeta {
            source_dir,
            pages,
            links,
        })
    }

    pub fn from_file(path: &Path) -> ReadResult<AtlasMeta> {
        let bytes = std::fs::read(path)
            .map_err(|e| ReadError::new(format!("read {}: {e}", path.display())))?;
        Self::from_bytes(&bytes)
    }

    /// Collects every frame id referenced by this atlas.
    pub fn frame_ids(&self) -> impl Iterator<Item = &str> {
        self.pages
            .iter()
            .flat_map(|p| p.frames.iter())
            .map(|f| f.id.as_str())
    }
}

/// A decoded atlas page: raw RGBA pixels plus its frame registry.
#[derive(Debug)]
pub struct AtlasPage {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// RGBA8, row-major, premultiplied as in the original pipeline.
    pub rgba: Vec<u8>,
    pub frames: Vec<Frame>,
}

impl AtlasPage {
    /// Decodes a `.data` texture blob (RLE format described at the top of this
    /// module).
    pub fn decode(name: String, bytes: &[u8]) -> ReadResult<AtlasPage> {
        let mut r = Reader::new(bytes);
        let width = r.read_u32()?;
        let height = r.read_u32()?;
        let has_alpha = r.read_u8()? != 0;

        let total_pixels = width as usize * height as usize;
        let mut rgba = vec![0u8; total_pixels * 4];
        let mut dst = 0usize;

        while dst < total_pixels * 4 {
            let run_pixels = r.read_u8()? as usize;
            if run_pixels == 0 {
                return Err(ReadError::new("zero-length pixel run"));
            }
            let run_bytes = run_pixels * 4;
            if dst + run_bytes > rgba.len() {
                return Err(ReadError::new("pixel run overflows texture"));
            }
            let (r_, g, b, a) = if has_alpha {
                let alpha = r.read_u8()?;
                if alpha > 0 {
                    let b = r.read_u8()?;
                    let g = r.read_u8()?;
                    let rr = r.read_u8()?;
                    (rr, g, b, alpha)
                } else {
                    (0, 0, 0, 0)
                }
            } else {
                let b = r.read_u8()?;
                let g = r.read_u8()?;
                let rr = r.read_u8()?;
                (rr, g, b, 255)
            };
            rgba[dst] = r_;
            rgba[dst + 1] = g;
            rgba[dst + 2] = b;
            rgba[dst + 3] = a;
            for i in (4..run_bytes).step_by(4) {
                rgba[dst + i] = r_;
                rgba[dst + i + 1] = g;
                rgba[dst + i + 2] = b;
                rgba[dst + i + 3] = a;
            }
            dst += run_bytes;
        }

        Ok(AtlasPage {
            name,
            width,
            height,
            rgba,
            frames: Vec::new(),
        })
    }
}

/// A loaded atlas: multiple texture pages plus a global id -> frame index.
#[derive(Debug, Default)]
pub struct Atlas {
    pub pages: Vec<AtlasPage>,
    /// Maps a **lowercase** frame id to a `(page index, frame index)`.
    /// Keys are always stored in lowercase for case-insensitive lookup.
    pub frame_index: HashMap<String, (usize, usize)>,
    /// Alias mapping: if a direct lookup fails, check `links` to redirect.
    links: HashMap<String, String>,
}

impl Atlas {
    /// Normalizes a frame id for case-insensitive lookup (lowercase).
    fn normalize(id: &str) -> String {
        id.to_ascii_lowercase()
    }

    /// Case-insensitive check: does this atlas contain a frame with the given id?
    #[must_use]
    pub fn has_frame(&self, id: &str) -> bool {
        self.frame_index.contains_key(&Self::normalize(id))
    }

    /// Case-insensitive lookup: resolve a frame id to `(page_idx, frame_idx)`.
    /// If the id isn't found directly, checks the LINKS alias mapping.
    #[must_use]
    pub fn resolve_frame(&self, id: &str) -> Option<(usize, usize)> {
        let key = Self::normalize(id);
        self.frame_index
            .get(&key)
            .or_else(|| {
                // Check LINKS alias: links maps alias → real id.
                self.links.get(&key).and_then(|real| {
                    self.frame_index.get(&Self::normalize(real))
                })
            })
            .copied()
    }

    /// Returns the alias mapping from the LINKS trailer.
    #[must_use]
    pub fn get_linked(&self, id: &str) -> Option<&str> {
        self.links.get(id).map(String::as_str)
    }

    /// Loads an atlas from a `.meta` file plus its numbered `.data` pages that
    /// live next to it (e.g. `Gameplay.meta` + `Gameplay0.data`).
    pub fn load(base: &Path) -> ReadResult<Atlas> {
        let dir = base.parent().unwrap_or_else(|| Path::new("."));
        let meta = AtlasMeta::from_file(base)?;

        let mut pages = Vec::with_capacity(meta.pages.len());
        for page in &meta.pages {
            let data_path = dir.join(format!("{}.data", page.name));
            let bytes = std::fs::read(&data_path)
                .map_err(|e| ReadError::new(format!("read {}: {e}", data_path.display())))?;
            let mut decoded = AtlasPage::decode(page.name.clone(), &bytes)?;
            decoded.frames = page.frames.clone();
            pages.push(decoded);
        }

        let mut frame_index = HashMap::new();
        for (pi, page) in pages.iter().enumerate() {
            for (fi, frame) in page.frames.iter().enumerate() {
                frame_index.insert(Self::normalize(&frame.id), (pi, fi));
            }
        }

        Ok(Atlas {
            pages,
            frame_index,
            links: meta.links,
        })
    }

    /// Loads a `PackerNoAtlas` atlas: each frame is its own `.data` texture
    /// stored under a page subdirectory (e.g. `Misc.meta` + `Misc/foo.data`).
    /// The clip rect is the whole texture; the offset still holds the untrimmed
    /// box so drawing behaves like the packed atlases.
    pub fn load_no_pack(base: &Path) -> ReadResult<Atlas> {
        let dir = base.parent().unwrap_or_else(|| Path::new("."));
        let meta = AtlasMeta::from_file(base)?;

        let mut pages = Vec::new();
        let mut frame_index = HashMap::new();
        for page in &meta.pages {
            for frame in &page.frames {
                let data_path = dir.join(&page.name).join(&frame.id).with_extension("data");
                let bytes = std::fs::read(&data_path)
                    .map_err(|e| ReadError::new(format!("read {}: {e}", data_path.display())))?;
                let mut decoded = AtlasPage::decode(frame.id.clone(), &bytes)?;
                let clip = FrameRect {
                    x: 0,
                    y: 0,
                    w: decoded.width as i16,
                    h: decoded.height as i16,
                };
                decoded.frames = vec![Frame {
                    id: frame.id.clone(),
                    clip,
                    offset: frame.offset,
                }];
                frame_index.insert(Self::normalize(&frame.id), (pages.len(), 0));
                pages.push(decoded);
            }
        }

        Ok(Atlas {
            pages,
            frame_index,
            links: meta.links,
        })
    }

    /// Appends another atlas's pages and frame index to this one (used to make
    /// the `Misc` no-pack atlas' backdrops available alongside `Gameplay`).
    pub fn merge(&mut self, other: Atlas) {
        let base = self.pages.len();
        for (id, (pi, fi)) in other.frame_index {
            self.frame_index.insert(id, (base + pi, fi));
        }
        for (k, v) in other.links {
            self.links.entry(k).or_insert(v);
        }
        self.pages.extend(other.pages);
    }

    /// Returns the RGBA8 pixel data of a single atlas frame (frame's own
    /// clip rect), as a contiguous row-major buffer.
    #[must_use]
    pub fn frame_rgba_into(&self, id: &str) -> Option<Vec<u8>> {
        let (pi, fi) = self.resolve_frame(id)?;
        let page = &self.pages[pi];
        let frame = &page.frames[fi];
        let w = frame.clip.w as usize;
        let h = frame.clip.h as usize;
        let start = ((frame.clip.y as usize) * page.width as usize + frame.clip.x as usize) * 4;
        let row_stride = page.width as usize * 4;
        let mut out = Vec::with_capacity(w * h * 4);
        for row in 0..h {
            let s = start + row * row_stride;
            out.extend_from_slice(&page.rgba[s..s + w * 4]);
        }
        Some(out)
    }

    /// Returns the clip rect of a frame, if known.
    #[must_use]
    pub fn frame_clip(&self, id: &str) -> Option<FrameRect> {
        let (pi, fi) = self.resolve_frame(id)?;
        Some(self.pages[pi].frames[fi].clip)
    }
}

/// Guesses the packer flavor of a single `.meta` atlas from our converted
/// directory layout:
/// * `Packer` (`Gameplay`) stores numbered page textures right next to the
///   meta (`Gameplay0.data`).
/// * `PackerNoAtlas` (`Misc`, each chapter's `CompleteScreens`) stores each
///   frame as its own texture under a subdirectory named after the page.
fn load_one(dir: &Path, meta_name: &str) -> Option<Atlas> {
    let base = dir.join(format!("{meta_name}.meta"));
    if !base.exists() {
        return None;
    }
    let meta = AtlasMeta::from_file(&base).ok()?;
    let first = meta.pages.first()?;
    // `Packer`: a page texture `<page.name>.data` sits next to the meta.
    let packed = dir.join(format!("{}.data", first.name)).is_file();
    let loaded = if packed {
        Atlas::load(&base).ok()?
    } else {
        Atlas::load_no_pack(&base).ok()?
    };
    Some(loaded)
}

/// Loads and merges every atlas in an `Atlases/` directory.
///
/// This is the pack-level entry point replacing the hardcoded
/// `Gameplay.meta` + `Misc.meta` couple in earlier versions: every converted
/// atlas in the directory becomes available to sprite/backdrop/plugin lookups.
///
/// Merge order matters: later atlases override earlier frame ids, so the two
/// most depended-on atlases are merged last.
///   `Gameplay`  — shared entity/tileset frames.
///   `Misc`      — no-pack backdrops.
/// Everything else (chapter `CompleteScreens` screenshots etc.) is merged
/// first; those ids (`00`, `01`, ...) never collide with `Gameplay`'s paths.
pub fn load_atlas_dir(dir: &Path) -> ReadResult<Atlas> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Err(ReadError::new(format!("read atlas dir {}", dir.display())));
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.strip_suffix(".meta").map(str::to_string)
        })
        .collect();
    names.sort();
    names.dedup();

    let mut atlas = Atlas::default();
    let mut failures = Vec::new();
    for name in &names {
        if name == "Gameplay" || name == "Misc" {
            continue;
        }
        match load_one(dir, name) {
            Some(a) => atlas.merge(a),
            None => failures.push(name.clone()),
        }
    }
    // Highest priority last.
    for name in ["Misc", "Gameplay"] {
        match load_one(dir, name) {
            Some(a) => atlas.merge(a),
            None => failures.push(name.to_string()),
        }
    }
    if !failures.is_empty() {
        eprintln!(
            "ruleste: failed to load {} atlas(es) from {}: {:?}",
            failures.len(),
            dir.display(),
            failures
        );
    }
    Ok(atlas)
}
