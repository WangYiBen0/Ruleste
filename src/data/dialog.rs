//! Dialog/cutscene text parser for Celeste's .txt format.
//!
//! The dialog format supports:
//! - Multi-page text with automatic line breaks
//! - Inline commands for styling (colors, font size, animations)
//! - Gameplay triggers and anchors
//! - Character portraits and voice
//!
//! File layout: keys are flush-left, values follow `KEY=VALUE`. A value
//! ending in `_` means the entry continues on the next non-empty line; the
//! continuation overwrites the same key (concatenating the stripped text).

use anyhow::{Context, Result};
use std::collections::HashMap;

/// Parsed dialog data from a .txt file.
#[derive(Debug)]
pub struct DialogData {
    /// Metadata entries (e.g., LANGUAGE, FONT, MENU_BEGIN)
    pub metadata: HashMap<String, String>,
    /// Dialog entries keyed by their name
    pub entries: HashMap<String, DialogEntry>,
}

/// A single dialog entry that can be displayed.
#[derive(Debug, Clone)]
pub struct DialogEntry {
    /// Internal name/key for this dialog
    pub name: String,
    /// Speaker name (optional, displayed above the box)
    pub speaker: Option<String>,
    /// Text content with commands
    pub text: String,
    /// Portrait ID (optional, for character face)
    pub portrait: Option<String>,
}

/// Heuristic: keys that look like dialog entry ids (UPPER_WITH_DIGITS)
/// rather than metadata (e.g., LANGUAGE, FONT, BEGIN, ORDER, ICON).
fn looks_like_dialog_key(key: &str) -> bool {
    // Dialog keys in Celeste are upper-case with underscores and often digits,
    // e.g. `CHAPTER1_INTRO`, `CH9_ENDING_A`. Metadata keys are short words
    // (LANGUAGE, FONT, BEGIN, ORDER, ICON) without digits.
    key.contains('_') && key.chars().any(|c| c.is_ascii_digit())
}

impl DialogData {
    /// Load a dialog .txt file from disk.
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read dialog file: {}", path.display()))?;

        Self::parse(&content)
    }

    /// Parse dialog content from a string.
    pub fn parse(content: &str) -> Result<Self> {
        let mut metadata: HashMap<String, String> = HashMap::new();
        let mut entries: HashMap<String, DialogEntry> = HashMap::new();
        // Track which entry we're currently building so continuation lines
        // (same key, value ending with `_`) append to it.
        let mut current_key: Option<String> = None;
        let mut current_is_entry = false;

        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines. An empty line ends the current
            // multi-line entry (per the format: newlines create page breaks).
            if line.is_empty() || line.starts_with('#') {
                current_key = None;
                current_is_entry = false;
                continue;
            }

            // Check if this is a key=value line
            if let Some(pos) = line.find('=') {
                let key = line[..pos].trim();
                let value = line[pos + 1..].trim();
                let continues = value.ends_with('_');
                let clean_value = if continues {
                    value[..value.len() - 1].to_string()
                } else {
                    value.to_string()
                };

                let is_entry_key = looks_like_dialog_key(key);

                if is_entry_key {
                    // Dialog entry
                    let entry = entries
                        .entry(key.to_string())
                        .or_insert_with(|| DialogEntry {
                            name: key.to_string(),
                            speaker: None,
                            text: String::new(),
                            portrait: None,
                        });

                    if entry.text.is_empty() {
                        entry.text.push_str(&clean_value);
                    } else {
                        entry.text.push(' ');
                        entry.text.push_str(&clean_value);
                    }

                    current_key = Some(key.to_string());
                    current_is_entry = true;
                } else {
                    // Metadata
                    let existing = metadata.get(key).cloned().unwrap_or_default();
                    if existing.is_empty() {
                        metadata.insert(key.to_string(), clean_value);
                    } else {
                        metadata.insert(key.to_string(), format!("{} {}", existing, clean_value));
                    }
                    current_key = Some(key.to_string());
                    current_is_entry = false;
                }
            } else {
                // Continuation line (no key). In the original format these
                // are indented and belong to the previous entry/metadata.
                if let Some(key) = &current_key {
                    let line_text = line.to_string();
                    if current_is_entry {
                        if let Some(entry) = entries.get_mut(key) {
                            if entry.text.is_empty() {
                                entry.text.push_str(&line_text);
                            } else {
                                entry.text.push(' ');
                                entry.text.push_str(&line_text);
                            }
                        }
                    } else if let Some(val) = metadata.get_mut(key) {
                        if val.is_empty() {
                            val.push_str(&line_text);
                        } else {
                            val.push(' ');
                            val.push_str(&line_text);
                        }
                    }
                }
            }
        }

        println!(
            "Loaded dialog: {} metadata entries, {} dialog entries",
            metadata.len(),
            entries.len()
        );

        Ok(Self { metadata, entries })
    }

    /// Get a dialog entry by name.
    pub fn get(&self, name: &str) -> Option<&DialogEntry> {
        self.entries.get(name)
    }

    /// Get a metadata value.
    pub fn get_metadata(&self, key: &str) -> Option<String> {
        self.metadata.get(key).cloned()
    }
}

/// Render text with inline commands processed.
///
/// This is a simplified renderer that strips most commands and renders
/// plain text. A full implementation would handle:
/// - Color changes {# RRBBGG}
/// - Font size {big}{/big}
/// - Speed changes {>> x}
/// - Text effects {~}{/~}{!}{/!}
/// - Newlines {n}
/// - Pauses {0.5}
pub struct DialogRenderer {
    pub current_page: usize,
    pub pages: Vec<String>,
    pub current_char: usize,
    pub display_timer: f32,
    /// Characters to display per second
    pub char_speed: f32,
}

impl DialogRenderer {
    pub fn new(entry: &DialogEntry) -> Self {
        let pages = Self::parse_into_pages(&entry.text);
        Self {
            current_page: 0,
            pages,
            current_char: 0,
            display_timer: 0.0,
            char_speed: 30.0, // Default: 30 chars/second
        }
    }

    /// Parse dialog text into pages (split by {n} commands and newlines).
    fn parse_into_pages(text: &str) -> Vec<String> {
        let mut pages = Vec::new();
        let mut current_page = String::new();

        // Process the text character-by-character, handling {n} and stripping
        // other commands. We iterate by bytes for {n}-style ASCII markers but
        // emit full chars to the buffer.
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '{' {
                // Collect until matching '}'
                let mut cmd = String::from('{');
                for cc in chars.by_ref() {
                    cmd.push(cc);
                    if cc == '}' {
                        break;
                    }
                }
                if cmd == "{n}" {
                    // Page break
                    if !current_page.is_empty() {
                        pages.push(current_page.trim().to_string());
                        current_page = String::new();
                    }
                }
                // Other commands ({...}) are stripped from display text
            } else if c == '\\' {
                // Escaped character: \# -> #, \. -> .
                if let Some(&next) = chars.peek() {
                    if next == '#' || next == '.' {
                        chars.next();
                        current_page.push(next);
                        continue;
                    }
                }
                current_page.push(c);
            } else {
                current_page.push(c);
            }
        }

        // Add the last page
        if !current_page.is_empty() {
            pages.push(current_page.trim().to_string());
        }

        pages
    }

    /// Update the display, advancing text based on timer.
    pub fn update(&mut self, dt: f32) -> bool {
        if self.current_page >= self.pages.len() {
            return true; // Complete
        }

        self.display_timer += dt;
        let chars_to_show = (self.display_timer * self.char_speed) as usize;
        self.current_char = chars_to_show;

        self.current_char >= self.pages[self.current_page].len()
    }

    /// Get the text to display for the current page (up to current_char).
    pub fn display_text(&self) -> &str {
        if self.current_page >= self.pages.len() {
            return "";
        }

        let page = &self.pages[self.current_page];
        if self.current_char >= page.len() {
            page
        } else {
            &page[..self.current_char]
        }
    }

    /// Advance to the next page.
    pub fn next_page(&mut self) -> bool {
        self.current_page += 1;
        self.current_char = 0;
        self.display_timer = 0.0;
        self.current_page >= self.pages.len()
    }

    /// Check if the current page is fully displayed.
    pub fn is_page_complete(&self) -> bool {
        if self.current_page >= self.pages.len() {
            return true;
        }
        self.current_char >= self.pages[self.current_page].len()
    }

    /// Check if the dialog is complete (all pages shown).
    pub fn is_complete(&self) -> bool {
        self.current_page >= self.pages.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_dialog() {
        let content = r#"
# Comment
MENU_BEGIN=CLIMB
MENU_EXIT=Exit
"#;
        let dialog = DialogData::parse(content).unwrap();

        assert_eq!(dialog.get_metadata("MENU_BEGIN"), Some("CLIMB".to_string()));
        assert_eq!(dialog.get_metadata("MENU_EXIT"), Some("Exit".to_string()));
    }

    #[test]
    fn test_parse_dialog_entry() {
        let content = r#"
MENU_BEGIN=CLIMB
CHAPTER1_INTRO=Welcome to Celeste!
"#;
        let dialog = DialogData::parse(content).unwrap();

        assert_eq!(dialog.get_metadata("MENU_BEGIN"), Some("CLIMB".to_string()));

        let entry = dialog.get("CHAPTER1_INTRO");
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().text, "Welcome to Celeste!");
    }

    #[test]
    fn test_parse_multiline() {
        let content = r#"
CHAPTER1_INTRO=Line one_
CHAPTER1_INTRO=Line two_
CHAPTER1_INTRO=Line three
"#;
        let dialog = DialogData::parse(content).unwrap();

        let entry = dialog.get("CHAPTER1_INTRO");
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().text, "Line one Line two Line three");
    }

    #[test]
    fn test_parse_pages() {
        let entry = DialogEntry {
            name: "test".to_string(),
            speaker: None,
            text: "First line{n}Second line{n}Third line".to_string(),
            portrait: None,
        };

        let renderer = DialogRenderer::new(&entry);
        assert_eq!(renderer.pages.len(), 3);
        assert_eq!(renderer.pages[0], "First line");
        assert_eq!(renderer.pages[1], "Second line");
        assert_eq!(renderer.pages[2], "Third line");
    }

    #[test]
    fn test_dialog_renderer_update() {
        let entry = DialogEntry {
            name: "test".to_string(),
            speaker: None,
            text: "Hello".to_string(),
            portrait: None,
        };

        let mut renderer = DialogRenderer::new(&entry);
        assert!(!renderer.is_complete());

        // After enough time, the page should be complete
        renderer.update(1.0); // 1 second = 30 chars
        assert!(renderer.is_page_complete());
        assert_eq!(renderer.display_text(), "Hello");
    }

    #[test]
    fn test_dialog_strips_commands() {
        let entry = DialogEntry {
            name: "test".to_string(),
            speaker: None,
            text: "Hello {# FF0000}world{#}!".to_string(),
            portrait: None,
        };

        let renderer = DialogRenderer::new(&entry);
        assert_eq!(renderer.pages.len(), 1);
        assert_eq!(renderer.pages[0], "Hello world!");
    }
}
