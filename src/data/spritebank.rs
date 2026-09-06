//! Parser for the original `Content/Graphics/Sprites.xml` sprite bank.
//!
//! Each `<SpriteName path="..." start="...">` element defines a sprite. Its
//! animations reference atlas subtexture prefixes (`path`), frame selections
//! (`frames="0,1"` / `frames="2-7"`) and optional `goto` transitions.
//! `Loop` animations restart; `Anim` animations play once.

use std::collections::HashMap;

/// Weighted random target for animation transitions.
/// Matches C#'s `Chooser<string>`: a list of `(weight, target)` pairs.
#[derive(Debug, Clone)]
pub struct Chooser {
    pub entries: Vec<ChooserEntry>,
}

#[derive(Debug, Clone)]
pub struct ChooserEntry {
    pub weight: f32,
    pub target: String,
}

impl Chooser {
    /// Picks a random target based on weights.
    #[must_use]
    pub fn choose(&self, rng: &mut u64) -> &str {
        let total: f32 = self.entries.iter().map(|e| e.weight).sum();
        if total <= 0.0 {
            return &self.entries[0].target;
        }
        // Simple xorshift64 PRNG step.
        *rng ^= *rng << 13;
        *rng ^= *rng >> 7;
        *rng ^= *rng << 17;
        let r = (*rng as f64 / u64::MAX as f64) as f32 * total;
        let mut acc = 0.0;
        for entry in &self.entries {
            acc += entry.weight;
            if r < acc {
                return &entry.target;
            }
        }
        &self.entries.last().unwrap().target
    }
}

/// Parses a goto attribute value like `"idle:10,flash:2,blink"`.
/// Returns a `Chooser` with weighted entries. Items without explicit weight
/// get weight 1.0 (matching C#'s `Chooser.FromString`).
fn parse_goto(s: &str) -> Chooser {
    let entries = s
        .split(',')
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            if let Some((name, w)) = part.split_once(':') {
                let weight = w.parse::<f32>().unwrap_or(1.0);
                Some(ChooserEntry {
                    weight,
                    target: name.to_string(),
                })
            } else {
                Some(ChooserEntry {
                    weight: 1.0,
                    target: part.to_string(),
                })
            }
        })
        .collect();
    Chooser { entries }
}

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
    /// `None` = no goto (loop or stop on last frame).
    /// `Some(chooser)` = pick next animation randomly.
    pub goto: Option<Chooser>,
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
    /// When set, the frame is centered on the entity anchor (`<Center/>` in
    /// the original SpriteBank, equivalent to `Justify 0.5 0.5`).
    pub center: bool,
    /// Optional explicit justify, `(x, y)` in `[0,1]` of the frame size
    /// (`<Justify x=".." y=".."/>` in the original SpriteBank).
    pub justify: Option<(f32, f32)>,
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
        let doc =
            roxmltree::Document::parse(xml).map_err(|e| anyhow::anyhow!("spritebank xml: {e}"))?;
        let root = doc.root_element();
        let mut sprites = HashMap::new();
        for node in root.children().filter(|n| n.is_element()) {
            if let Some(sprite) = parse_sprite(node, &sprites)? {
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

fn parse_sprite(
    el: roxmltree::Node<'_, '_>,
    existing: &HashMap<String, SpriteData>,
) -> anyhow::Result<Option<SpriteData>> {
    let name = el.tag_name().name();
    // Skip the Sprites root itself and any metadata nodes without a path.
    if name == "Sprites" || name.starts_with('#') {
        return Ok(None);
    }
    let path = el.attribute("path").unwrap_or("").to_string();
    if path.is_empty() {
        return Ok(None);
    }

    // If `copy` is present, start from the referenced sprite's data.
    let mut sprite = el
        .attribute("copy")
        .and_then(|src| existing.get(src).cloned());

    // Parse child elements (Origin, Center, Justify, Anim, Loop).
    let mut origin = (0, 0);
    let mut center = false;
    let mut justify = None;
    let mut animations = HashMap::new();

    for child in el.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "Origin" => {
                origin = (
                    child.attribute("x").and_then(parse_i32).unwrap_or(0),
                    child.attribute("y").and_then(parse_i32).unwrap_or(0),
                );
            }
            "Center" => {
                center = true;
            }
            "Justify" => {
                let jx = child.attribute("x").and_then(parse_f32).unwrap_or(0.5);
                let jy = child.attribute("y").and_then(parse_f32).unwrap_or(0.5);
                justify = Some((jx, jy));
            }
            "Anim" | "Loop" => {
                let id = child.attribute("id").unwrap_or("").to_string();
                let anim_path = child.attribute("path").unwrap_or(&id).to_string();
                let delay = child.attribute("delay").and_then(parse_f32).unwrap_or(0.1);
                let frames = parse_frames(child.attribute("frames"));
                let goto = child.attribute("goto").map(parse_goto);
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

    // Merge: current element's animations override the copied ones.
    let source_name = name.to_string();
    let start = el.attribute("start").unwrap_or("idle").to_string();
    let start = if let Some(ref s) = sprite {
        // Only override start if the copy element explicitly specified one.
        if el.attribute("start").is_some() || !s.start.is_empty() && start != "idle" {
            start
        } else {
            s.start.clone()
        }
    } else {
        start
    };
    let origin = if el.attribute("x").is_some() || el.attribute("y").is_some() {
        origin
    } else if let Some(ref s) = sprite {
        if el.children().filter(|n| n.is_element()).any(|n| n.tag_name().name() == "Origin") {
            origin
        } else {
            s.origin
        }
    } else {
        origin
    };
    let center = center
        || sprite.as_ref().is_some_and(|s| {
            // Inherit center from source unless overridden.
            s.center
                && !el
                    .children()
                    .filter(|n| n.is_element())
                    .any(|n| n.tag_name().name() == "Center")
        });
    let justify = if el
        .children()
        .filter(|n| n.is_element())
        .any(|n| n.tag_name().name() == "Justify")
    {
        justify
    } else if let Some(ref s) = sprite {
        s.justify
    } else {
        justify
    };

    // If copied, merge animations: source + current (current overrides).
    if let Some(ref mut s) = sprite {
        for (k, v) in animations.drain() {
            s.animations.insert(k, v);
        }
        s.name = source_name;
        s.path = path;
        s.start = start;
        s.origin = origin;
        s.center = center;
        s.justify = justify;
        return Ok(Some(s.clone()));
    }

    Ok(Some(SpriteData {
        name: source_name,
        path,
        start,
        origin,
        center,
        justify,
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
