//! Map backdrop layers: the parallax textures in `Style > Backgrounds` /
//! `Style > Foregrounds`.
//!
//! Mirrors Celeste's `MapData.CreateBackdrops`/`ParseBackdrop` and
//! `Parallax.Render`: a backdrop has a world position, a per-axis scroll
//! factor relative to the camera, optional speed drift and color/alpha, and
//! loops either horizontally and/or vertically.

use ruleste_plugin_api::types::Vec2;

use crate::data::binary_packer::Element;

#[derive(Debug, Clone)]
pub struct Backdrop {
    /// Atlas frame id of the texture to draw (e.g. "bgs/00/bg0").
    pub texture: String,
    /// World-space position (the backdrop's anchor).
    pub position: Vec2,
    /// Per-axis scroll factor (0..1 for parallax backgrounds).
    pub scroll: Vec2,
    /// World-units-per-second drift applied each frame.
    pub speed: Vec2,
    /// RGB color (0-255), multiplied by `alpha` when drawn.
    pub color: (u8, u8, u8),
    /// Alpha multiplier applied to the color.
    pub alpha: f32,
    pub flip_x: bool,
    pub flip_y: bool,
    pub loop_x: bool,
    pub loop_y: bool,
    /// `blendmode="additive"` draws with additive blending.
    pub additive: bool,
}

impl Backdrop {
    /// Applies `Parallax.Update`: drift the anchor by `speed * dt`.
    pub fn update(&mut self, dt: f32) {
        self.position.x += self.speed.x * dt;
        self.position.y += self.speed.y * dt;
    }
}

/// Parses `Style > Backgrounds` / `Style > Foregrounds` into backdrop lists,
/// following `MapData.CreateBackdrops` (including `apply` attribute groups).
/// Only `parallax` backdrops are supported; shader-based types (snow, stars,
/// ...) are skipped with a warning.
pub fn parse(style: Option<&Element>) -> (Vec<Backdrop>, Vec<Backdrop>) {
    let mut backgrounds = Vec::new();
    let mut foregrounds = Vec::new();
    let Some(style) = style else {
        return (backgrounds, foregrounds);
    };
    for child in &style.children {
        if child.name == "Backgrounds" {
            parse_group(child, &mut backgrounds);
        } else if child.name == "Foregrounds" {
            parse_group(child, &mut foregrounds);
        }
    }
    (backgrounds, foregrounds)
}

fn parse_group(group: &Element, out: &mut Vec<Backdrop>) {
    for child in &group.children {
        if child.name.eq_ignore_ascii_case("apply") {
            // An `apply` element sets attributes inherited by its children.
            for inner in &child.children {
                if let Some(b) = parse_backdrop(inner, Some(child)) {
                    out.push(b);
                }
            }
        } else if let Some(b) = parse_backdrop(child, None) {
            out.push(b);
        }
    }
}

fn parse_backdrop(child: &Element, above: Option<&Element>) -> Option<Backdrop> {
    if !child.name.eq_ignore_ascii_case("parallax") {
        eprintln!(
            "ruleste: skipping unsupported backdrop type {:?}",
            child.name
        );
        return None;
    }

    let texture = child.attr_str("texture", "");
    if texture.is_empty() {
        return None;
    }

    // Attribute lookup mirrors `ParseBackdrop`: the child wins, then `apply`.
    let get = |name: &str, default: f32| -> f32 {
        if child.attr(name).is_some() {
            child.attr_f32(name, default)
        } else if let Some(a) = above {
            a.attr_f32(name, default)
        } else {
            default
        }
    };
    let get_color = |name: &str| -> Option<(u8, u8, u8)> {
        let v = if child.attr(name).is_some() {
            Some(child.attr_str(name, ""))
        } else {
            above.and_then(|a| a.attr(name).is_some().then(|| a.attr_str(name, "")))
        };
        v.and_then(|s| hex_color(&s))
    };

    let position = Vec2::new(get("x", 0.0), get("y", 0.0));
    let scroll = Vec2::new(get("scrollx", 1.0), get("scrolly", 1.0));
    let speed = Vec2::new(get("speedx", 0.0), get("speedy", 0.0));
    let color = get_color("color").unwrap_or((255, 255, 255));
    let alpha = get("alpha", 1.0);

    let blend = {
        let raw = if child.attr("blendmode").is_some() {
            child.attr_str("blendmode", "alphablend")
        } else {
            above
                .and_then(|a| {
                    a.attr("blendmode")
                        .map(|_| a.attr_str("blendmode", "alphablend"))
                })
                .unwrap_or_else(|| "alphablend".to_string())
        };
        raw.eq_ignore_ascii_case("additive")
    };

    Some(Backdrop {
        texture,
        position,
        scroll,
        speed,
        color,
        alpha,
        flip_x: child.attr_bool("flipx", false),
        flip_y: child.attr_bool("flipy", false),
        loop_x: child.attr_bool("loopx", true),
        loop_y: child.attr_bool("loopy", true),
        additive: blend,
    })
}

/// Parses an RRGGBB hex color string into an `(r, g, b)` tuple.
fn hex_color(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some((
        ((v >> 16) & 0xff) as u8,
        ((v >> 8) & 0xff) as u8,
        (v & 0xff) as u8,
    ))
}
