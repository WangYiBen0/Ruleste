//! Parser for the original `Content/Graphics/Sprites.xml` sprite bank.
//!
//! Each `<SpriteName path="..." start="...">` element defines a sprite. Its
//! animations reference atlas subtexture prefixes (`path`), frame selections
//! (`frames="0,1"` / `frames="2-7"`) and optional `goto` transitions.
//! `Loop` animations restart; `Anim` animations play once.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Animation {
    pub id: String,
    /// Frame prefix relative to the sprite's `path` (e.g. "idle").
    pub path: String,
    /// Seconds per frame.
    pub delay: f32,
    /// Explicit frame indices; empty means the full `00..` sequence.
    pub frames: Vec<u32>,
    /// Animation to switch to when this one finishes.
    pub goto: Option<String>,
    /// Whether this animation loops.
    pub is_loop: bool,
}

#[derive(Debug, Clone)]
pub struct SpriteData {
    pub name: String,
    /// Atlas path prefix for this sprite, e.g. "characters/player/".
    pub path: String,
    /// Initial animation id.
    pub start: String,
    /// Texture origin (usually bottom-center of the first frame).
    pub origin: (i32, i32),
    pub animations: HashMap<String, Animation>,
}

impl SpriteData {
    #[must_use]
    pub fn animation(&self, id: &str) -> Option<&Animation> {
        self.animations.get(id)
    }

    /// Resolves the atlas subtexture prefix for an animation.
    #[must_use]
    pub fn texture_prefix(&self, anim: &Animation) -> String {
        format!("{}{}", self.path, anim.path)
    }
}

#[derive(Debug, Clone, Default)]
pub struct SpriteBank {
    pub sprites: HashMap<String, SpriteData>,
}

impl SpriteBank {
    pub fn from_xml(xml: &str) -> anyhow::Result<SpriteBank> {
        let doc = roxmltree::Document::parse(xml)
            .map_err(|e| anyhow::anyhow!("spritebank xml: {e}"))?;
        let root = doc.root_element();
        let mut sprites = HashMap::new();
        for node in root.children().filter(|n| n.is_element()) {
            if let Some(sprite) = parse_sprite(node)? {
                sprites.insert(sprite.name.clone(), sprite);
            }
        }
        Ok(SpriteBank { sprites })
    }

    pub fn load(path: &std::path::Path) -> anyhow::Result<SpriteBank> {
        let xml = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("read {}: {e}", path.display()))?;
        Self::from_xml(&xml)
    }

    #[must_use]
    pub fn sprite(&self, name: &str) -> Option<&SpriteData> {
        self.sprites.get(name)
    }
}

fn parse_sprite(el: roxmltree::Node<'_, '_>) -> anyhow::Result<Option<SpriteData>> {
    let name = el.tag_name().name();
    // Skip the Sprites root itself and any metadata nodes without a path.
    if name == "Sprites" || name.starts_with('#') {
        return Ok(None);
    }
    let path = el.attribute("path").unwrap_or("").to_string();
    if path.is_empty() {
        return Ok(None);
    }
    let start = el.attribute("start").unwrap_or("idle").to_string();
    let mut origin = (0, 0);
    let mut animations = HashMap::new();

    for child in el.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "Origin" => {
                origin = (
                    child.attribute("x").and_then(parse_i32).unwrap_or(0),
                    child.attribute("y").and_then(parse_i32).unwrap_or(0),
                );
            }
            "Anim" | "Loop" => {
                let id = child.attribute("id").unwrap_or("").to_string();
                let anim_path = child.attribute("path").unwrap_or(&id).to_string();
                let delay = child.attribute("delay").and_then(parse_f32).unwrap_or(0.1);
                let frames = parse_frames(child.attribute("frames"));
                let goto = child.attribute("goto").map(str::to_string);
                let is_loop = child.tag_name().name() == "Loop";
                animations.insert(
                    id.clone(),
                    Animation {
                        id,
                        path: anim_path,
                        delay,
                        frames,
                        goto,
                        is_loop,
                    },
                );
            }
            _ => {}
        }
    }

    Ok(Some(SpriteData {
        name: name.to_string(),
        path,
        start,
        origin,
        animations,
    }))
}

fn parse_i32(s: &str) -> Option<i32> {
    s.parse().ok()
}

fn parse_f32(s: &str) -> Option<f32> {
    s.parse().ok()
}

fn parse_frames(s: Option<&str>) -> Vec<u32> {
    let mut out = Vec::new();
    let Some(s) = s else { return out };
    for part in s.split(',') {
        let part = part.trim();
        if let Some((a, b)) = part.split_once('-') {
            if let (Ok(lo), Ok(hi)) = (a.parse::<u32>(), b.parse::<u32>()) {
                out.extend(lo..=hi);
            }
        } else if let Ok(v) = part.parse::<u32>() {
            out.push(v);
        }
    }
    out
}
