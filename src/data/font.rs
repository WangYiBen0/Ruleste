//! XNA .xnb font asset parser.
//!
//! XNB (XNA Binary) is the compiled asset format for XNA games. The SpriteFont
//! type contains glyph metrics (character rectangle and kerning) for text
//! rendering.

use anyhow::{Context, Result};
use std::collections::HashMap;

#[derive(Debug)]
pub struct SpriteFont {
    pub texture_name: String,
    pub glyphs: HashMap<char, Glyph>,
    pub line_spacing: i32,
    pub spacing: i32,
    /// Whether the font uses kerning (pairwise character spacing adjustments).
    pub use_kerning: bool,
    /// Default character to use for missing glyphs.
    pub default_character: Option<char>,
}

#[derive(Debug, Clone)]
pub struct Glyph {
    /// Character code point.
    pub char: char,
    /// Bounding box in the texture atlas (pixels).
    pub bounds: GlyphBounds,
    /// Cropping rectangle for the glyph (pixels removed from the original).
    pub cropping: GlyphCropping,
    /// X-offset to advance to the next character.
    pub advance: i32,
    /// Left side bearing (horizontal padding before the glyph).
    pub left_bearing: i32,
    /// Right side bearing (horizontal padding after the glyph).
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
    /// Load a .xnb SpriteFont file from disk.
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read font file: {}", path.display()))?;

        let mut reader = crate::data::reader::Reader::new(&data);

        // XNB header: "XNBw" (magic) + version + flags + compressed size (optional)
        let magic = reader.take(4)?;
        if magic != b"XNBw" {
            anyhow::bail!("Invalid XNB magic: expected 'XNBw', got {:?}", magic);
        }

        let _target_platform = reader.read_u8()?;
        let version = reader.read_u8()?;
        let flags = reader.read_u8()?;
        let _is_compressed = (flags & 0x80) != 0;

        // XNA 4.0 uses version 5
        if version != 5 {
            anyhow::bail!(
                "Unsupported XNB version: {} (expected 5 for XNA 4.0)",
                version
            );
        }

        // Compressed size field is present only if bit 0x80 is set
        if (flags & 0x80) != 0 {
            let _compressed_size = reader.read_u32()?;
        }

        // Skip the type reader string (e.g., "Microsoft.Xna.Framework.Content.SpriteFontReader...")
        reader.read_dotnet_string()?;

        // Read the shared resource count
        let _shared_resources = reader.read_u32()?;

        // For now, we'll use a simpler approach: parse the .spritefont XML if available
        // and use the .xnb just as a marker. Full XNB parsing is complex.
        // Look for a companion .spritefont file.
        let xml_path = path.with_extension("spritefont");
        if xml_path.exists() {
            Self::load_from_xml(&xml_path)
        } else {
            anyhow::bail!(
                "Font XML file not found at {}. Full XNB parsing not implemented.",
                xml_path.display()
            );
        }
    }

    /// Load from the XNA .spritefont XML format.
    fn load_from_xml(path: &std::path::Path) -> Result<Self> {
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

        // Default ASCII range if no regions specified
        if char_regions.is_empty() {
            char_regions.push((' ', '~'));
        }

        // Build glyphs for each character in the regions
        let mut glyphs = HashMap::new();
        for (start, end) in char_regions {
            for c in start..=end {
                // For now, create placeholder glyphs with reasonable metrics
                // A real implementation would read these from a texture atlas
                glyphs.insert(c, create_placeholder_glyph(c, size as i32));
            }
        }

        // Calculate line spacing (typically 1.2x the font size)
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

    /// Get a glyph for a character, using the default character if not found.
    pub fn glyph(&self, c: char) -> Option<&Glyph> {
        self.glyphs
            .get(&c)
            .or_else(|| self.default_character.and_then(|def| self.glyphs.get(&def)))
    }

    /// Measure the width of a string.
    pub fn measure_string(&self, text: &str) -> i32 {
        let mut width = 0;
        let mut prev_char: Option<char> = None;

        for c in text.chars() {
            if let Some(glyph) = self.glyph(c) {
                // Apply kerning if enabled and we have a previous character
                if self.use_kerning {
                    // Simplified kerning: just add spacing
                    if prev_char.is_some() {
                        width += self.spacing;
                    }
                }

                width += glyph.advance;
            }
            prev_char = Some(c);
        }

        width
    }

    /// Calculate the number of lines for wrapped text.
    pub fn line_count(&self, text: &str, max_width: i32) -> usize {
        let mut lines = 1;
        let mut current_width = 0;

        for word in text.split_whitespace() {
            let word_width = self.measure_string(word);
            if current_width == 0 {
                // First word on line
                if word_width > max_width {
                    // Word is longer than line - must wrap anyway
                    lines += (word_width as f32 / max_width as f32).ceil() as usize;
                    current_width = word_width % max_width;
                } else {
                    current_width = word_width;
                }
            } else if current_width + self.spacing + word_width <= max_width {
                // Word fits on current line
                current_width += self.spacing + word_width;
            } else {
                // Need to wrap
                lines += 1;
                current_width = word_width;
            }
        }

        lines
    }
}

/// Parse HTML entity like &#32; to a character.
fn parse_html_entity(s: &str) -> Result<char> {
    let s = s.trim();
    if s.starts_with("&#") && s.ends_with(';') {
        let num_str = &s[2..s.len() - 1];
        let code_point: u32 = num_str.parse().context("Invalid HTML entity number")?;
        std::char::from_u32(code_point)
            .ok_or_else(|| anyhow::anyhow!("Invalid Unicode code point: {}", code_point))
    } else {
        // Handle simple escape like \#
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

/// Create a placeholder glyph with reasonable metrics for the given font size.
fn create_placeholder_glyph(c: char, font_size: i32) -> Glyph {
    // Estimate glyph width based on character (monospace-ish)
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
        advance: width + 1, // Add 1 pixel for spacing
        left_bearing: 0,
        right_bearing: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_html_entity() {
        assert_eq!(parse_html_entity("&#32;").unwrap(), ' ');
        assert_eq!(parse_html_entity("&#65;").unwrap(), 'A');
        assert_eq!(parse_html_entity("&#126;").unwrap(), '~');
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

        // Add a simple glyph for 'H'
        let h_glyph = create_placeholder_glyph('H', 12);
        font.glyphs.insert('H', h_glyph);

        let width = font.measure_string("HH");
        assert!(width > 0);
    }
}
