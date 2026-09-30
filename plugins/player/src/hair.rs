//! Player hair, mirroring `Celeste/PlayerHair.cs`.
//!
//! Madeline's hair is a short chain of nodes that trails her head. Node 0 is
//! pinned to the sprite's hair anchor (offset by the frame's `HairOffset`
//! metadata); every following node chases the one before it with a distance
//! constraint, so the chain whips around when she turns or dashes.
//!
//! Rendering follows the original's two passes: a black border drawn as four
//! offset copies of every node (front to back), then the hair itself drawn back
//! to front so node 0 (the bangs) lands on top.

use ruleste_plugins_api::host::FrameMeta;
use ruleste_plugins_api::types::Color;
use ruleste_plugins_api::types::Vec2;

/// The atlas frame for the hair strands (`PlayerHair.Hair`).
const HAIR_FRAME: &str = "characters/player/hair00";
/// The bangs frame prefix; the metadata's `bangs` index selects the suffix.
const BANGS_PREFIX: &str = "characters/player/bangs";
/// `PlayerSprite.HairCount`.
const HAIR_COUNT: usize = 4;
/// `PlayerHair.StepPerSegment`.
const STEP_PER_SEGMENT: Vec2 = Vec2 { x: 0.0, y: 2.0 };
/// `PlayerHair.StepInFacingPerSegment`.
const STEP_IN_FACING_PER_SEGMENT: f32 = 0.5;
/// `PlayerHair.StepApproach`.
const STEP_APPROACH: f32 = 64.0;
/// The maximum length of a segment, `PlayerHair.AfterUpdate`'s `num = 3f`.
const MAX_SEGMENT: f32 = 3.0;
/// The hair anchor's fixed vertical offset from the sprite position,
/// `-9f * Sprite.Scale.Y` in the original (scale is 1 for the player).
const ANCHOR_Y: f32 = -9.0;
/// `Player.NormalHairColor`.
const HAIR_COLOR: Color = Color {
    r: 0xE4,
    g: 0x9C,
    b: 0x3C,
    a: 0xFF,
};
/// `PlayerHair.Border` is `Color.Black`.
const BORDER_COLOR: Color = Color {
    r: 0x00,
    g: 0x00,
    b: 0x00,
    a: 0xFF,
};

/// The hair state, serialized alongside the rest of the player.
#[derive(Debug, Clone, Copy)]
pub struct Hair {
    /// The chain of node positions, in world space.
    pub nodes: [Vec2; HAIR_COUNT],
    /// `PlayerHair.wave`, the phase advancing the idle sway.
    pub wave: f32,
    /// False until the first `after_update`, so `start` can seed the nodes
    /// off-screen exactly like the original does.
    pub started: bool,
    /// `PlayerHair.SimulateMotion`: when false the chain holds still instead of
    /// chasing its target (used during cutscenes and the death animation).
    pub simulate_motion: bool,
}

impl Default for Hair {
    fn default() -> Self {
        Hair {
            nodes: [Vec2::ZERO; HAIR_COUNT],
            wave: 0.0,
            started: false,
            simulate_motion: true,
        }
    }
}

impl Hair {
    /// `PlayerHair.Start`: every node begins far above and behind the player,
    /// so the hair visibly settles in rather than snapping to place.
    pub fn start(&mut self, position: Vec2, facing: i32) {
        let seed = Vec2 {
            x: position.x + (0 - facing) as f32 * 200.0,
            y: position.y + 200.0,
        };
        self.nodes = [seed; HAIR_COUNT];
        self.started = true;
    }

    /// `PlayerHair.AfterUpdate`: pin node 0 to the head, then let the rest of
    /// the chain chase it with an approach plus a hard length limit.
    ///
    /// `meta` is the current frame's `HairOffset`; `facing` is `Facings`.
    pub fn after_update(&mut self, position: Vec2, facing: i32, meta: &FrameMeta, dt: f32) {
        let hair_offset = Vec2 {
            x: meta.hair_offset.0 as f32 * facing as f32,
            y: meta.hair_offset.1 as f32,
        };
        self.nodes[0] = Vec2 {
            x: position.x + hair_offset.x,
            y: position.y + ANCHOR_Y + hair_offset.y,
        };

        let f = facing as f32;
        // The first segment steps twice as far as the rest, matching the
        // `* 2f` on `StepInFacingPerSegment` at the head of the chain.
        let mut target = add(
            self.nodes[0],
            Vec2 {
                x: (0.0 - f) * STEP_IN_FACING_PER_SEGMENT * 2.0,
                y: self.wave.sin() * 0.0,
            },
        );
        target = add(target, STEP_PER_SEGMENT);

        let mut prev = self.nodes[0];
        for i in 1..HAIR_COUNT {
            if self.simulate_motion {
                // The tail reacts more slowly than the head: the approach rate
                // decays to half by the last node.
                let rate = (1.0 - (i as f32 / HAIR_COUNT as f32) * 0.5) * STEP_APPROACH;
                self.nodes[i] = approach(self.nodes[i], target, rate * dt);
            }
            // Clamp the segment length so the hair cannot stretch.
            if length(sub(self.nodes[i], prev)) > MAX_SEGMENT {
                let dir = safe_normalize(sub(self.nodes[i], prev));
                self.nodes[i] = add(prev, scale(dir, MAX_SEGMENT));
            }
            target = add(
                self.nodes[i],
                Vec2 {
                    x: (0.0 - f) * STEP_IN_FACING_PER_SEGMENT,
                    // Each node sways on its own phase, offset by `i * 0.8f`.
                    y: (self.wave + i as f32 * 0.8).sin() * 0.0,
                },
            );
            target = add(target, STEP_PER_SEGMENT);
            prev = self.nodes[i];
        }
    }
}

/// `PlayerHair.Update`: advance the sway phase at 4 radians per second.
pub fn advance_wave(wave: &mut f32, dt: f32) {
    *wave += dt * 4.0;
}

/// `PlayerHair.GetHairScale`: the head keeps its full width (mirrored by the
/// facing), while the tail tapers toward 0.25.
fn hair_scale(i: usize, facing: i32) -> Vec2 {
    let taper = 0.25 + (1.0 - i as f32 / HAIR_COUNT as f32) * 0.75;
    Vec2 {
        x: if i == 0 { facing as f32 } else { taper },
        y: taper,
    }
}

/// The hair texture's anchor, `new Vector2(5f, 5f)` in `PlayerHair.Render`.
const ORIGIN: Vec2 = Vec2 { x: 5.0, y: 5.0 };

/// Blits a hair frame at `position` with the original's `(5,5)` origin and the
/// per-node taper from `GetHairScale`.
fn draw(frame: &str, position: Vec2, scale: Vec2, color: Color) {
    ruleste_plugins_api::host::draw_image_color_scaled(
        frame,
        position.x - ORIGIN.x,
        position.y - ORIGIN.y,
        scale.x,
        scale.y,
        color,
    );
}

/// Draws the hair, mirroring `PlayerHair.Render`.
///
/// The bangs frame is selected by the current sprite's metadata, which is what
/// `PlayerHair.Render` reads from `Sprite.HairFrame`. Call this before the
/// player sprite so the strands sit behind her head, as the original's
/// component render order does.
pub fn render(hair: &Hair, facing: i32, meta: &FrameMeta) {
    if !meta.has_hair {
        return;
    }
    let mut nodes = hair.nodes;
    nodes[0].x = nodes[0].x.floor();
    nodes[0].y = nodes[0].y.floor();
    let bangs = bangs_frame(meta.bangs_frame);

    if BORDER_COLOR.a > 0 {
        for i in 0..HAIR_COUNT {
            let frame = if i == 0 { &bangs } else { HAIR_FRAME };
            for offset in [
                Vec2 { x: -1.0, y: 0.0 },
                Vec2 { x: 1.0, y: 0.0 },
                Vec2 { x: 0.0, y: -1.0 },
                Vec2 { x: 0.0, y: 1.0 },
            ] {
                draw(
                    frame,
                    add(nodes[i], offset),
                    hair_scale(i, facing),
                    BORDER_COLOR,
                );
            }
        }
    }
    for i in (0..HAIR_COUNT).rev() {
        let frame = if i == 0 { &bangs } else { HAIR_FRAME };
        draw(frame, nodes[i], hair_scale(i, facing), HAIR_COLOR);
    }
}

/// The `characters/player/bangsNN` frame for a metadata bangs index.
fn bangs_frame(index: i32) -> String {
    format!("{BANGS_PREFIX}{index:02}")
}

// ---------------------------------------------------------------------------
// Vector helpers
// ---------------------------------------------------------------------------

fn add(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 {
        x: a.x + b.x,
        y: a.y + b.y,
    }
}

fn sub(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}

fn scale(v: Vec2, k: f32) -> Vec2 {
    Vec2 {
        x: v.x * k,
        y: v.y * k,
    }
}

fn length(v: Vec2) -> f32 {
    (v.x * v.x + v.y * v.y).sqrt()
}

/// `Vector2.SafeNormalize`: a zero vector stays zero instead of becoming NaN.
fn safe_normalize(v: Vec2) -> Vec2 {
    let len = length(v);
    if len <= f32::EPSILON {
        Vec2::ZERO
    } else {
        scale(v, 1.0 / len)
    }
}

/// `Calc.Approach`.
fn approach(value: Vec2, target: Vec2, max_move: f32) -> Vec2 {
    let delta = sub(target, value);
    if length(delta) <= max_move {
        target
    } else {
        add(value, scale(safe_normalize(delta), max_move))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(offset: (i32, i32), bangs: i32) -> FrameMeta {
        FrameMeta {
            has_hair: true,
            hair_offset: offset,
            bangs_frame: bangs,
            carry_y_offset: 0,
        }
    }

    #[test]
    fn hair_count_and_constants_match_player_hair_cs() {
        assert_eq!(HAIR_COUNT, 4);
        assert_eq!(STEP_PER_SEGMENT.y, 2.0);
        assert_eq!(STEP_IN_FACING_PER_SEGMENT, 0.5);
        assert_eq!(STEP_APPROACH, 64.0);
        assert_eq!(MAX_SEGMENT, 3.0);
        assert_eq!(ANCHOR_Y, -9.0);
    }

    #[test]
    fn start_seeds_every_node_above_and_behind_the_player() {
        let mut hair = Hair::default();
        hair.start(Vec2 { x: 100.0, y: 200.0 }, -1);
        let expected = Vec2 { x: 300.0, y: 400.0 };
        for n in hair.nodes {
            assert_eq!(n, expected, "every node starts at the same seed");
        }
    }

    #[test]
    fn after_update_pins_node_zero_to_the_hair_anchor() {
        let mut hair = Hair::default();
        hair.start(Vec2 { x: 100.0, y: 200.0 }, 1);
        // `HairOffset` is (2, -1) and facing is Right, so the anchor is
        // `position + (2 * facing, -9 + -1)`.
        hair.after_update(
            Vec2 { x: 100.0, y: 200.0 },
            1,
            &meta((2, -1), 0),
            1.0 / 60.0,
        );
        assert_eq!(hair.nodes[0], Vec2 { x: 102.0, y: 190.0 });
    }

    #[test]
    fn hair_offset_is_mirrored_by_facing() {
        let mut hair = Hair::default();
        hair.start(Vec2 { x: 0.0, y: 0.0 }, -1);
        // Facing Left flips the X component of the offset, matching
        // `Sprite.HairOffset * new Vector2((float)Facing, 1f)`.
        hair.after_update(Vec2 { x: 0.0, y: 0.0 }, -1, &meta((2, -1), 0), 1.0 / 60.0);
        assert_eq!(hair.nodes[0].x, -2.0);
        assert_eq!(hair.nodes[0].y, -10.0);
    }

    #[test]
    fn segments_never_exceed_the_maximum_length() {
        let mut hair = Hair::default();
        // Start the chain taut and far from where the head will be, so the
        // clamp is what pulls the nodes back in.
        hair.nodes = [Vec2 { x: 500.0, y: 500.0 }; HAIR_COUNT];
        for _ in 0..30 {
            hair.after_update(Vec2 { x: 0.0, y: 0.0 }, 1, &meta((0, -2), 0), 1.0 / 60.0);
        }
        for i in 1..HAIR_COUNT {
            let seg = length(sub(hair.nodes[i], hair.nodes[i - 1]));
            assert!(seg <= MAX_SEGMENT + 1e-3, "segment {i} stretched to {seg}");
        }
    }

    #[test]
    fn hair_eventually_settles_behind_the_player() {
        let mut hair = Hair::default();
        hair.start(Vec2 { x: 0.0, y: 0.0 }, 1);
        for _ in 0..120 {
            hair.after_update(Vec2 { x: 0.0, y: 0.0 }, 1, &meta((0, -2), 0), 1.0 / 60.0);
        }
        // Node 0 is pinned to the head anchor (y = 0 + -9 + -2).
        assert_eq!(hair.nodes[0], Vec2 { x: 0.0, y: -11.0 });
        // Facing Right, so the rest of the chain trails to the left of the
        // head and hangs down by the two pixels per segment.
        for (i, n) in hair.nodes.iter().enumerate().skip(1) {
            assert!(n.x <= 0.0, "node {i} drifted forward to {}", n.x);
            // Three segments of `StepPerSegment` below the head, plus the
            // per-segment facing step pulling them back up.
            assert!(
                (-11.0..=0.0).contains(&n.y),
                "node {i} settled at {}, outside the hanging range",
                n.y
            );
        }
    }

    #[test]
    fn scale_tapers_from_head_to_tail() {
        // `GetHairScale`: the head is full width (mirrored), the tail tapers
        // toward 0.25.
        assert_eq!(hair_scale(0, 1).x, 1.0);
        assert_eq!(hair_scale(0, -1).x, -1.0);
        assert_eq!(hair_scale(1, 1).y, 0.25 + 0.75 * (1.0 - 0.25));
        let last = hair_scale(HAIR_COUNT - 1, 1);
        assert!(last.y < hair_scale(1, 1).y, "the tail is thinner");
    }

    #[test]
    fn bangs_frame_index_is_zero_padded() {
        assert_eq!(bangs_frame(0), "characters/player/bangs00");
        assert_eq!(bangs_frame(2), "characters/player/bangs02");
    }
}
