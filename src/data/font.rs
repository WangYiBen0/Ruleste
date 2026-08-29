//! Text assets: BMFont (`.fnt`) descriptor parser + PNG page decoder,
//! and a backwards-compatible XNA `.xnb` / `.spritefont` placeholder used by
//! the in-game font loader.
//!
//! Mirrors Monocle's `PixelFont` / `PixelFontSize` / `PixelFontCharacter`. A
//! font is a collection of *sizes* (e.g. 32/64/192), each declaring its own
//! texture page, character glyphs and kerning table. The renderer picks the
//! smallest size >= the requested `baseSize * max(scale)`, then scales the
//! result by `baseSize / that_size`.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

// ---------------------------------------------------------------------------
// PixelFont: the real, BMFont-driven pipeline (Celeste's `PixelFont`).
// ---------------------------------------------------------------------------

/// One glyph in a font size, matching Monocle's `PixelFontCharacter`.
#[derive(Debug, Clone)]
pub struct PixelFontCharacter {
    pub character: char,
    /// Region in the page texture, in pixels.
    pub region: GlyphRegion,
    /// Offset from the pen position to the glyph's top-left, in pixels.
    pub x_offset: i32,
    pub y_offset: i32,
    /// How far to advance the pen after this glyph, in pixels.
    pub x_advance: i32,
    /// Per-following-character kerning adjustments.
    pub kerning: HashMap<char, i32>,
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphRegion {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// One of N size tables for a font. Mirrors `PixelFontSize`.
#[derive(Debug, Clone)]
pub struct PixelFontSize {
    /// Face size from `<info size="...">`.
    pub size: f32,
    /// Line height in pixels (`<common lineHeight>`).
    pub line_height: i32,
    /// Per-page texture file basename (e.g. `renogare64_0`).
    pub page_textures: Vec<String>,
    pub characters: HashMap<char, PixelFontCharacter>,
    /// True if this entry is the outline pass (added by Everest; Celeste
    /// itself only ships the regular pass).
    pub outline: bool,
}

impl PixelFontSize {
    /// Width and height of `text` after layout. Mirrors
    /// `PixelFontSize.Measure(string)`.
    pub fn measure(&self, text: &str) -> (i32, i32) {
        if text.is_empty() {
            return (0, 0);
        }
        let mut max_w = 0i32;
        let mut cur_w = 0i32;
        let mut total_h = self.line_height;
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\n' {
                if cur_w > max_w {
                    max_w = cur_w;
                }
                cur_w = 0;
                total_h += self.line_height;
                continue;
            }
            if let Some(g) = self.characters.get(&c) {
                cur_w += g.x_advance;
                if let Some(&next) = chars.peek() {
                    if let Some(&k) = g.kerning.get(&next) {
                        cur_w += k;
                    }
                }
            }
        }
        if cur_w > max_w {
            max_w = cur_w;
        }
        (max_w, total_h)
    }

    /// Width of the substring of `text` starting at byte index `start` until
    /// the next newline. Mirrors `PixelFontSize.WidthToNextLine`.
    pub fn width_to_next_line(&self, text: &str, start: usize) -> i32 {
        let chars: Vec<char> = text.chars().collect();
        let mut w = 0i32;
        let mut i = start;
        while i < chars.len() && chars[i] != '\n' {
            if let Some(g) = self.characters.get(&chars[i]) {
                w += g.x_advance;
                if i + 1 < chars.len() {
                    if let Some(&k) = g.kerning.get(&chars[i + 1]) {
                        w += k;
                    }
                }
            }
            i += 1;
        }
        w
    }

    /// Total height of `text` given the current `line_height`.
    pub fn height_of(&self, text: &str) -> i32 {
        if text.is_empty() {
            return 0;
        }
        let mut lines = 1;
        for c in text.chars() {
            if c == '\n' {
                lines += 1;
            }
        }
        lines * self.line_height
    }
}

/// A font with one or more size tables, mirroring Monocle's `PixelFont`.
#[derive(Debug, Clone)]
pub struct PixelFont {
    pub face: String,
    pub sizes: Vec<PixelFontSize>,
}

impl PixelFont {
    pub fn new(face: impl Into<String>) -> Self {
        Self {
            face: face.into(),
            sizes: Vec::new(),
        }
    }

    /// Pick the smallest size whose `size` field is >= the requested
    /// `base_size * max(scale)`. If the largest is still smaller, return the
    /// largest. Mirrors `PixelFont.Get`.
    pub fn get(&self, base_size: f32) -> Option<&PixelFontSize> {
        if self.sizes.is_empty() {
            return None;
        }
        for s in &self.sizes {
            if s.size >= base_size {
                return Some(s);
            }
        }
        self.sizes.last()
    }

    /// Load a font from a BMFont XML descriptor. The matching texture pages
    /// are *not* read here; `Renderer::upload_font_page` is called for each
    /// `page_textures[i]` once the atlas is in memory.
    pub fn load(path: &Path) -> Result<Self> {
        let xml_str = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read font file: {}", path.display()))?;
        Self::parse(&xml_str)
    }

    /// Parse a BMFont XML string. Exposed separately for tests.
    pub fn parse(xml_str: &str) -> Result<Self> {
        let doc = roxmltree::Document::parse(xml_str).context("Failed to parse font XML")?;
        let root = doc.root_element();

        let info = root
            .children()
            .find(|n| n.tag_name().name() == "info")
            .context("font: missing <info>")?;
        let face = info
            .attribute("face")
            .context("font: <info> missing face attribute")?
            .to_string();

        let common = root
            .children()
            .find(|n| n.tag_name().name() == "common")
            .context("font: missing <common>")?;
        let line_height: i32 = common
            .attribute("lineHeight")
            .context("font: <common> missing lineHeight")?
            .parse()
            .context("font: invalid lineHeight")?;

        let mut page_textures: Vec<String> = Vec::new();
        if let Some(pages) = root.children().find(|n| n.tag_name().name() == "pages") {
            for page in pages.children() {
                if page.tag_name().name() != "page" {
                    continue;
                }
                if let Some(file) = page.attribute("file") {
                    let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
                    page_textures.push(stem.to_string());
                }
            }
        }

        let mut characters: HashMap<char, PixelFontCharacter> = HashMap::new();
        if let Some(chars_node) = root.children().find(|n| n.tag_name().name() == "chars") {
            for ch in chars_node.children() {
                if ch.tag_name().name() != "char" {
                    continue;
                }
                let parse_attr = |key: &str| -> Result<i32> {
                    ch.attribute(key)
                        .ok_or_else(|| anyhow::anyhow!("font: <char> missing {key}"))
                        .and_then(|v| v.parse().map_err(Into::into))
                };
                let id = parse_attr("id")? as u32;
                let Some(character) = std::char::from_u32(id) else {
                    continue;
                };
                let region = GlyphRegion {
                    x: parse_attr("x")?,
                    y: parse_attr("y")?,
                    width: parse_attr("width")?,
                    height: parse_attr("height")?,
                };
                let glyph = PixelFontCharacter {
                    character,
                    region,
                    x_offset: ch
                        .attribute("xoffset")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0),
                    y_offset: ch
                        .attribute("yoffset")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0),
                    x_advance: ch
                        .attribute("xadvance")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(region.width),
                    kerning: HashMap::new(),
                };
                characters.insert(character, glyph);
            }
        }

        if let Some(kern) = root.children().find(|n| n.tag_name().name() == "kernings") {
            for k in kern.children() {
                if k.tag_name().name() != "kerning" {
                    continue;
                }
                let first_attr = k.attribute("first").and_then(|v| v.parse::<u32>().ok());
                let second_attr = k.attribute("second").and_then(|v| v.parse::<u32>().ok());
                let amount: Option<i32> = k.attribute("amount").and_then(|v| v.parse().ok());
                let (Some(first), Some(second), Some(amount)) = (first_attr, second_attr, amount)
                else {
                    continue;
                };
                let Some(c) = std::char::from_u32(first) else {
                    continue;
                };
                let Some(n) = std::char::from_u32(second) else {
                    continue;
                };
                if let Some(glyph) = characters.get_mut(&c) {
                    glyph.kerning.insert(n, amount);
                }
            }
        }

        let size: f32 = info
            .attribute("size")
            .and_then(|v| v.parse().ok())
            .unwrap_or(line_height as f32);

        let font_size = PixelFontSize {
            size,
            line_height,
            page_textures,
            characters,
            outline: false,
        };

        let mut font = Self::new(face);
        font.sizes.push(font_size);
        Ok(font)
    }
}

// ---------------------------------------------------------------------------
// SpriteFont: backwards-compat placeholder used by the in-game font loader.
// The real game would use PixelFont; SpriteFont survives as a stub so the
// rest of the renderer (which still consumes its metrics) keeps compiling.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SpriteFont {
    pub texture_name: String,
    pub glyphs: HashMap<char, Glyph>,
    pub line_spacing: i32,
    pub spacing: i32,
    pub use_kerning: bool,
    pub default_character: Option<char>,
}

#[derive(Debug, Clone)]
pub struct Glyph {
    pub char: char,
    pub bounds: GlyphBounds,
    pub cropping: GlyphCropping,
    pub advance: i32,
    pub left_bearing: i32,
    pub right_bearing: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphCropping {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl SpriteFont {
    /// Load an XNA `.xnb` SpriteFont. Falls back to a companion `.spritefont`
    /// XML when present, which contains the actual metrics.
    pub fn load(path: &Path) -> Result<Self> {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read font file: {}", path.display()))?;
        let mut reader = crate::data::reader::Reader::new(&data);
        let magic = reader.take(4)?;
        if magic != b"XNBw" {
            anyhow::bail!("Invalid XNB magic: expected 'XNBw', got {:?}", magic);
        }
        let _target_platform = reader.read_u8()?;
        let version = reader.read_u8()?;
        let flags = reader.read_u8()?;
        if (flags & 0x80) != 0 {
            let _compressed_size = reader.read_u32()?;
        }
        if version != 5 {
            anyhow::bail!(
                "Unsupported XNB version: {} (expected 5 for XNA 4.0)",
                version
            );
        }
        reader.read_dotnet_string()?;
        let _shared_resources = reader.read_u32()?;
        let xml_path = path.with_extension("spritefont");
        if xml_path.exists() {
            Self::load_from_xml(&xml_path)
        } else {
            anyhow::bail!(
                "Font XML file not found at {}. Full XNB parsing not implemented.",
                xml_path.display()
            )
        }
    }

    fn load_from_xml(path: &Path) -> Result<Self> {
        let xml_str = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read font XML: {}", path.display()))?;
        let doc = roxmltree::Document::parse(&xml_str).context("Failed to parse font XML")?;
        let root = doc.root();
        let asset = root
            .descendants()
            .find(|n| n.tag_name().name() == "Asset")
            .context("No Asset element found")?;

        let mut font_name = String::new();
        let mut size: f32 = 12.0;
        let mut spacing: i32 = 0;
        let mut use_kerning = true;
        let mut style = "Regular".to_string();
        let mut default_character: Option<char> = None;
        let mut char_regions: Vec<(char, char)> = Vec::new();

        for child in asset.children() {
            match child.tag_name().name() {
                "FontName" => {
                    font_name = child.text().unwrap_or("").to_string();
                }
                "Size" => {
                    size = child.text().unwrap_or("12").parse().unwrap_or(12.0);
                }
                "Spacing" => {
                    spacing = child.text().unwrap_or("0").parse().unwrap_or(0);
                }
                "UseKerning" => {
                    use_kerning = child.text().unwrap_or("true") == "true";
                }
                "Style" => {
                    style = child.text().unwrap_or("Regular").to_string();
                }
                "DefaultCharacter" => {
                    if let Some(text) = child.text() {
                        default_character = text.chars().next();
                    }
                }
                "CharacterRegions" => {
                    for region in child.children() {
                        if region.tag_name().name() == "CharacterRegion" {
                            let start = region
                                .children()
                                .find(|n| n.tag_name().name() == "Start")
                                .and_then(|n| n.text())
                                .and_then(|t| parse_html_entity(t).ok());
                            let end = region
                                .children()
                                .find(|n| n.tag_name().name() == "End")
                                .and_then(|n| n.text())
                                .and_then(|t| parse_html_entity(t).ok());
                            if let (Some(s), Some(e)) = (start, end) {
                                char_regions.push((s, e));
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if char_regions.is_empty() {
            char_regions.push((' ', '~'));
        }

        let mut glyphs = HashMap::new();
        for (start, end) in char_regions {
            for c in start..=end {
                glyphs.insert(c, create_placeholder_glyph(c, size as i32));
            }
        }

        let line_spacing = (size as i32 * 12 / 10).max(1);

        println!(
            "Loaded font '{}' (size={}, style={}, {} glyphs, spacing={}, kerning={})",
            font_name,
            size,
            style,
            glyphs.len(),
            spacing,
            use_kerning
        );

        Ok(SpriteFont {
            texture_name: font_name,
            glyphs,
            line_spacing,
            spacing,
            use_kerning,
            default_character,
        })
    }

    pub fn glyph(&self, c: char) -> Option<&Glyph> {
        self.glyphs
            .get(&c)
            .or_else(|| self.default_character.and_then(|def| self.glyphs.get(&def)))
    }

    pub fn measure_string(&self, text: &str) -> i32 {
        let mut width = 0;
        let mut prev_char: Option<char> = None;
        for c in text.chars() {
            if let Some(glyph) = self.glyph(c) {
                if self.use_kerning && prev_char.is_some() {
                    width += self.spacing;
                }
                width += glyph.advance;
            }
            prev_char = Some(c);
        }
        width
    }

    pub fn line_count(&self, text: &str, max_width: i32) -> usize {
        let mut lines = 1;
        let mut current_width = 0;
        for word in text.split_whitespace() {
            let word_width = self.measure_string(word);
            if current_width == 0 {
                if word_width > max_width {
                    lines += (word_width as f32 / max_width as f32).ceil() as usize;
                    current_width = word_width % max_width;
                } else {
                    current_width = word_width;
                }
            } else if current_width + self.spacing + word_width <= max_width {
                current_width += self.spacing + word_width;
            } else {
                lines += 1;
                current_width = word_width;
            }
        }
        lines
    }
}

fn parse_html_entity(s: &str) -> Result<char> {
    let s = s.trim();
    if s.starts_with("&#") && s.ends_with(';') {
        let num_str = &s[2..s.len() - 1];
        let code_point: u32 = num_str.parse().context("Invalid HTML entity number")?;
        std::char::from_u32(code_point)
            .ok_or_else(|| anyhow::anyhow!("Invalid Unicode code point: {}", code_point))
    } else {
        let mut chars = s.chars();
        if chars.next() == Some('\\') {
            chars
                .next()
                .ok_or_else(|| anyhow::anyhow!("Invalid escape sequence"))
        } else {
            chars.next().ok_or_else(|| anyhow::anyhow!("Empty string"))
        }
    }
}

fn create_placeholder_glyph(c: char, font_size: i32) -> Glyph {
    let width = if c.is_whitespace() {
        font_size / 2
    } else if c == 'i' || c == 'l' || c == 't' {
        font_size / 4
    } else if c == 'm' || c == 'M' || c == 'w' || c == 'W' {
        font_size * 3 / 4
    } else {
        font_size / 2
    };
    Glyph {
        char: c,
        bounds: GlyphBounds {
            x: 0,
            y: 0,
            width,
            height: font_size,
        },
        cropping: GlyphCropping {
            x: 0,
            y: 0,
            width,
            height: font_size,
        },
        advance: width + 1,
        left_bearing: 0,
        right_bearing: 0,
    }
}

// ---------------------------------------------------------------------------
// PNG page loader: decodes a font's texture page into the ABGR byte order
// `Renderer` uses (matches the software-renderer quirk documented in
// AGENTS.md). Returns `None` if the file is missing or the decode fails —
// the renderer treats that as "font not drawn" and stays silent.
// ---------------------------------------------------------------------------

/// A decoded font page: the ABGR byte buffer plus dimensions.
#[derive(Debug, Clone)]
pub struct FontPage {
    pub width: u32,
    pub height: u32,
    /// Pixel data in `R, G, B, A` byte order. The renderer's
    /// `PixelFormat::ABGR8888` upload path flips channels for SDL's
    /// little-endian interpretation, so this is what callers should hand it.
    pub rgba: Vec<u8>,
}

/// Resolve `page_basename` to a sibling PNG of the `.fnt` file, decode it,
/// and return its pixels in `R, G, B, A` order.
pub fn load_page_png(fnt_path: &Path, page_basename: &str) -> Result<Option<FontPage>> {
    let dir = fnt_path
        .parent()
        .context("font XML has no parent directory")?;
    let png_path = dir.join(format!("{page_basename}.png"));
    if !png_path.exists() {
        return Ok(None);
    }
    let img = image::open(&png_path)
        .with_context(|| format!("Failed to decode font page {}", png_path.display()))?
        .to_rgba8();
    Ok(Some(FontPage {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_FNT: &str = r#"<?xml version="1.0"?>
<font>
  <info face="Renogare" size="64" bold="0" italic="0" charset="" unicode="1" stretchH="100" smooth="1" aa="4" padding="0,0,0,0" spacing="1,1" outline="0"/>
  <common lineHeight="64" base="48" scaleW="2048" scaleH="2048" pages="1" packed="0" alphaChnl="0" redChnl="4" greenChnl="4" blueChnl="4"/>
  <pages>
    <page id="0" file="renogare64_0.png" />
  </pages>
  <chars count="3">
    <char id="72" x="1701" y="82" width="31" height="34" xoffset="3" yoffset="14" xadvance="37" page="0" chnl="15" />
    <char id="73" x="355" y="130" width="8" height="34" xoffset="4" yoffset="14" xadvance="16" page="0" chnl="15" />
    <char id="10" x="0" y="0" width="0" height="0" xoffset="0" yoffset="0" xadvance="0" page="0" chnl="15" />
  </chars>
  <kernings count="1">
    <kerning first="72" second="73" amount="-1" />
  </kernings>
</font>"#;

    #[test]
    fn parses_face_and_line_height() {
        let f = PixelFont::parse(SAMPLE_FNT).unwrap();
        assert_eq!(f.face, "Renogare");
        assert_eq!(f.sizes.len(), 1);
        assert_eq!(f.sizes[0].line_height, 64);
        assert_eq!(f.sizes[0].size, 64.0);
        assert_eq!(f.sizes[0].page_textures, vec!["renogare64_0"]);
    }

    #[test]
    fn parses_chars_and_kerning() {
        let f = PixelFont::parse(SAMPLE_FNT).unwrap();
        let h = f.sizes[0].characters.get(&'H').expect("H");
        assert_eq!(h.x_advance, 37);
        assert_eq!(h.region.width, 31);
        let kern = f.sizes[0]
            .characters
            .get(&'H')
            .unwrap()
            .kerning
            .get(&'I')
            .copied();
        assert_eq!(kern, Some(-1));
    }

    #[test]
    fn measure_single_line() {
        let f = PixelFont::parse(SAMPLE_FNT).unwrap();
        let (w, h) = f.sizes[0].measure("HI");
        assert_eq!(w, 37 + (-1) + 16);
        assert_eq!(h, 64);
    }

    #[test]
    fn measure_multiline_picks_widest() {
        let f = PixelFont::parse(SAMPLE_FNT).unwrap();
        // "HI": H adv=37, I adv=16, kern=-1 → 52; "I": 16. Max = 52.
        let (w, h) = f.sizes[0].measure("HI\nI");
        assert_eq!(w, 52);
        assert_eq!(h, 128);
    }

    #[test]
    fn width_to_next_line_stops_at_newline() {
        let f = PixelFont::parse(SAMPLE_FNT).unwrap();
        let size = &f.sizes[0];
        assert_eq!(size.width_to_next_line("HI\nI", 0), 52);
        assert_eq!(size.width_to_next_line("HI\nI", 3), 16);
    }

    #[test]
    fn height_of_counts_newlines() {
        let f = PixelFont::parse(SAMPLE_FNT).unwrap();
        let size = &f.sizes[0];
        assert_eq!(size.height_of("A"), 64);
        assert_eq!(size.height_of("A\nB"), 128);
        assert_eq!(size.height_of("A\nB\nC"), 192);
    }

    #[test]
    fn get_picks_smallest_size_at_least_requested() {
        let mut f = PixelFont::new("X");
        f.sizes.push(PixelFontSize {
            size: 32.0,
            line_height: 32,
            page_textures: vec!["x32".into()],
            characters: HashMap::new(),
            outline: false,
        });
        f.sizes.push(PixelFontSize {
            size: 64.0,
            line_height: 64,
            page_textures: vec!["x64".into()],
            characters: HashMap::new(),
            outline: false,
        });
        assert_eq!(f.get(40.0).unwrap().size, 64.0);
        assert_eq!(f.get(64.0).unwrap().size, 64.0);
        assert_eq!(f.get(20.0).unwrap().size, 32.0);
    }

    #[test]
    fn test_parse_html_entity() {
        assert_eq!(parse_html_entity("&#32;").unwrap(), ' ');
        assert_eq!(parse_html_entity("&#65;").unwrap(), 'A');
        assert_eq!(parse_html_entity("&#126;").unwrap(), '~');
    }

    #[test]
    fn test_load_page_png_missing_is_none() {
        // Use a valid fnt path (the one that exists) with a non-existent page basename.
        let fnt = Path::new("src/data/font.rs");
        let result = load_page_png(fnt, "this_page_definitely_does_not_exist_xyz");
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_measure_string() {
        let mut font = SpriteFont {
            texture_name: "test".to_string(),
            glyphs: std::collections::HashMap::new(),
            line_spacing: 14,
            spacing: 0,
            use_kerning: true,
            default_character: None,
        };
        let h_glyph = create_placeholder_glyph('H', 12);
        font.glyphs.insert('H', h_glyph);
        let width = font.measure_string("HH");
        assert!(width > 0);
    }
}
