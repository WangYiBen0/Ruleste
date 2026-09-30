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
        if self.entries.is_empty() {
            // Defensive: an empty `goto` (or one that parsed to nothing)
            // shouldn't panic; the caller falls back to looping/stopping.
            return "";
        }
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

/// Per-frame metadata for one player-sprite frame, mirroring
/// `PlayerSprite.PlayerAnimMetadata`.
///
/// The original builds this from a `<Metadata>` block inside the sprite's
/// `<SpriteName>` element:
///
/// ```xml
/// <Frames path="walk" hair="0,-1|0,-1|0,-1|0,-3|0,-2" carry="0,0,0,0"/>
/// ```
///
/// `hair` is a `|`-separated list, one entry per frame of that animation. Each
/// entry is either `x` (or empty) for "this frame has no hair", or
/// `offsetX,offsetY[:bangsFrame]`. `carry` is a `,`-separated list of the
/// vertical offsets used when Madeline carries something.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FrameMetadata {
    /// False when the frame opts out of hair (the `x`/empty entry form).
    pub has_hair: bool,
    /// Hair anchor offset from the sprite's render position, in pixels.
    pub hair_offset: (i32, i32),
    /// Which `characters/player/bangsNN` frame to use for this frame.
    pub bangs_frame: i32,
    /// Vertical carry offset; `0` when the frame does not list one.
    pub carry_y_offset: i32,
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
    /// Per-frame metadata keyed by the full atlas frame id, e.g.
    /// `characters/player/idle00`. Mirrors `PlayerSprite.FrameMetadata`, which
    /// is keyed by `Texture.AtlasPath` in the original.
    pub frame_metadata: HashMap<String, FrameMetadata>,
}

impl SpriteBank {
    /// The metadata for a resolved atlas frame id, if the sprite declared any.
    #[must_use]
    pub fn frame_meta(&self, frame_id: &str) -> Option<&FrameMetadata> {
        self.frame_metadata.get(frame_id)
    }
}

impl SpriteBank {
    pub fn from_xml(xml: &str) -> anyhow::Result<SpriteBank> {
        let doc =
            roxmltree::Document::parse(xml).map_err(|e| anyhow::anyhow!("spritebank xml: {e}"))?;
        let root = doc.root_element();
        let mut sprites = HashMap::new();
        let mut frame_metadata = HashMap::new();
        for node in root.children().filter(|n| n.is_element()) {
            if let Some(sprite) = parse_sprite(node, &sprites)? {
                sprites.insert(sprite.name.clone(), sprite);
            }
            // `PlayerSprite.CreateFramesMetadata` walks each sprite's
            // `<Metadata>` block after the sprite itself is parsed, so the
            // frame ids it builds (`path` + `<Frames path=...>`) resolve against
            // the same `path` attribute the sprite uses.
            if let Some(path) = node.attribute("path").filter(|p| !p.is_empty()) {
                collect_frame_metadata(node, path, &mut frame_metadata);
            }
        }
        Ok(SpriteBank {
            sprites,
            frame_metadata,
        })
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
    // Skip the Sprites root itself, and `<Metadata>` blocks: those hold
    // per-frame `hair`/`carry` data, not animation definitions, and are
    // collected separately by `collect_frame_metadata`.
    if name == "Sprites" || name == "Metadata" || name.starts_with('#') {
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
        if el
            .children()
            .filter(|n| n.is_element())
            .any(|n| n.tag_name().name() == "Origin")
        {
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

/// Walks a sprite element's `<Metadata>` block and fills `out` with one entry
/// per declared frame, keyed by the full atlas frame id.
///
/// Mirrors `PlayerSprite.CreateFramesMetadata`:
///
/// ```csharp
/// string[] hair   = item.Attr("hair").Split('|');
/// string[] carry  = item.Attr("carry", "").Split(',');
/// for (int i = 0; i < Math.Max(hair.Length, carry.Length); i++) {
///     string frame = text + ((i < 10) ? "0" : "") + i;
///     if (i == 0 && !GFX.Game.Has(frame)) frame = text;
///     ...
/// }
/// ```
///
/// Note the zero-padding rule: index 0 becomes `...00`, and a *single*
/// unsuffixed frame is addressed as the bare prefix when the padded name does
/// not exist in the atlas. Since the parser cannot see the atlas, it registers
/// the padded name and the bare prefix for index 0, letting the lookup miss
/// harmlessly.
fn collect_frame_metadata(
    el: roxmltree::Node<'_, '_>,
    path: &str,
    out: &mut HashMap<String, FrameMetadata>,
) {
    let Some(metadata) = el
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "Metadata")
    else {
        return;
    };

    for frames in metadata
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "Frames")
    {
        let base = format!("{path}{}", frames.attribute("path").unwrap_or(""));
        // `Attr` returns "" for a missing attribute, and `"".Split('|')` in C#
        // yields a single empty entry — so a missing `hair` still produces one
        // frame slot, which the loop below turns into "no hair".
        let hair: Vec<&str> = frames.attribute("hair").unwrap_or("").split('|').collect();
        let carry: Vec<&str> = frames.attribute("carry").unwrap_or("").split(',').collect();

        for i in 0..hair.len().max(carry.len()) {
            let mut meta = parse_hair_entry(hair.get(i).copied().unwrap_or(""));
            meta.carry_y_offset = carry
                .get(i)
                .map(|c| c.trim().parse::<i32>().unwrap_or(0))
                .unwrap_or(0);

            let padded = format!("{base}{i:02}");
            out.insert(padded.clone(), meta);
            if i == 0 {
                // The original falls back to the unsuffixed frame when the
                // padded name is absent; register it too so either lookup works.
                out.insert(base.clone(), meta);
            }
        }
    }
}

/// Parses one `hair="x,y[:frame]"` entry into its offset and bangs frame.
///
/// An entry of `x` (or an empty one) means "no hair", reported as
/// `has_hair: false` by the caller.
fn parse_hair_entry(entry: &str) -> FrameMetadata {
    let entry = entry.trim();
    if entry.is_empty() || entry.eq_ignore_ascii_case("x") {
        return FrameMetadata::default();
    }
    let (offset, frame) = match entry.split_once(':') {
        Some((offset, frame)) => (offset, frame.trim().parse::<i32>().unwrap_or(0)),
        None => (entry, 0),
    };
    // The offsets are written as `0, -2`, so the split tolerates inner spaces.
    let mut parts = offset.split(',');
    let x = parts.next().map(str::trim).and_then(parse_i32).unwrap_or(0);
    let y = parts.next().map(str::trim).and_then(parse_i32).unwrap_or(0);
    FrameMetadata {
        has_hair: true,
        hair_offset: (x, y),
        bangs_frame: frame,
        carry_y_offset: 0,
    }
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
        if let Some((frame, count)) = part.split_once('*') {
            // Celeste's SpriteBank uses `index*count` to hold a frame for a
            // number of animation ticks. Preserve that repetition exactly;
            // dropping it changes the timing and, for repeat-only lists, can
            // make the whole animation fall back to an unrelated sequential
            // frame list.
            if let (Ok(index), Ok(count)) =
                (frame.trim().parse::<u32>(), count.trim().parse::<usize>())
            {
                out.extend(std::iter::repeat_n(index, count));
            }
        } else if let Some((a, b)) = part.split_once('-') {
            if let (Ok(lo), Ok(hi)) = (a.parse::<u32>(), b.parse::<u32>()) {
                out.extend(lo..=hi);
            }
        } else if let Ok(v) = part.parse::<u32>() {
            out.push(v);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_frames_supports_repeat_ranges_and_mixed_lists() {
        assert_eq!(parse_frames(Some("0*3")), vec![0, 0, 0]);
        assert_eq!(parse_frames(Some("0*3,0-2")), vec![0, 0, 0, 0, 1, 2]);
        assert_eq!(parse_frames(Some("7*2,3,1-2")), vec![7, 7, 3, 1, 2]);
        assert_eq!(parse_frames(Some("8*10,8-17")).len(), 20);
        assert!(parse_frames(None).is_empty());
    }

    /// A `<Metadata>` block shaped like the real `player` sprite, exercising
    /// the offset, the bangs-frame suffix, the hair opt-out and the carry list.
    const PLAYER_XML: &str = r#"
    <Sprites>
      <SpriteName name="player" path="characters/player/">
        <Metadata>
          <Frames path="idle" hair="0,-2|0,-2:1|x|0,-1:2" carry="0,0,-1"/>
          <Frames path="walk" hair="0,-1|0,-3"/>
        </Metadata>
      </SpriteName>
    </Sprites>"#;

    #[test]
    fn frame_metadata_is_keyed_by_padded_frame_id() {
        let bank = SpriteBank::from_xml(PLAYER_XML).unwrap();

        // Frame 0's entry is registered under both the padded and the bare
        // name, mirroring the original's unsuffixed fallback.
        let idle00 = bank.frame_meta("characters/player/idle00").unwrap();
        assert!(idle00.has_hair);
        assert_eq!(idle00.hair_offset, (0, -2));
        assert_eq!(idle00.bangs_frame, 0, "no suffix means bangs frame 0");

        let idle01 = bank.frame_meta("characters/player/idle01").unwrap();
        assert_eq!(idle01.hair_offset, (0, -2));
        assert_eq!(
            idle01.bangs_frame, 1,
            "the `:1` suffix selects the bangs frame"
        );

        let idle03 = bank.frame_meta("characters/player/idle03").unwrap();
        assert_eq!(idle03.bangs_frame, 2);
    }

    #[test]
    fn frame_metadata_handles_hair_opt_out_and_carry_offsets() {
        let bank = SpriteBank::from_xml(PLAYER_XML).unwrap();

        // `x` opts the frame out of hair entirely.
        let idle02 = bank.frame_meta("characters/player/idle02").unwrap();
        assert!(!idle02.has_hair);
        assert_eq!(idle02.hair_offset, (0, 0));

        // `carry` is a separate, comma-separated list; frame 2 of `idle` has no
        // hair but does carry a -1 offset.
        assert_eq!(idle02.carry_y_offset, -1);
        assert_eq!(
            bank.frame_meta("characters/player/walk00")
                .unwrap()
                .carry_y_offset,
            0
        );

        // A frame the metadata never mentions has no entry at all.
        assert!(bank.frame_meta("characters/player/run00").is_none());
    }

    #[test]
    fn frame_metadata_tolerates_spaces_in_offsets() {
        // The real bank writes `hair="1, 1|1,2"`, with a space after the comma.
        let bank = SpriteBank::from_xml(
            r#"<Sprites><SpriteName name="p" path="characters/player/">
                 <Metadata><Frames path="runStumble" hair="1, 1|1,2"/></Metadata>
               </SpriteName></Sprites>"#,
        )
        .unwrap();
        assert_eq!(
            bank.frame_meta("characters/player/runStumble00")
                .unwrap()
                .hair_offset,
            (1, 1)
        );
        assert_eq!(
            bank.frame_meta("characters/player/runStumble01")
                .unwrap()
                .hair_offset,
            (1, 2)
        );
    }
}
