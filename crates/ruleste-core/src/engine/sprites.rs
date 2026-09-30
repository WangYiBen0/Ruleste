//! Sprite animation playback. Ticks each entity's animation frame forward and
//! handles loop/goto transitions, mirroring `Monocle.Sprite`.

use std::collections::HashMap;

use crate::data::atlas::Atlas;
use crate::data::spritebank::{Animation, SpriteBank, SpriteData};
use crate::engine::ecs::World;

pub struct SpriteAnimator<'a> {
    atlas: &'a Atlas,
    bank: &'a SpriteBank,
    /// Cache of (sprite name, animation id) -> resolved atlas frame ids.
    cache: HashMap<(String, String), Vec<String>>,
    /// Simple xorshift64 PRNG for weighted random goto selection.
    rng: u64,
}

impl<'a> SpriteAnimator<'a> {
    pub fn new(atlas: &'a Atlas, bank: &'a SpriteBank) -> SpriteAnimator<'a> {
        SpriteAnimator {
            atlas,
            bank,
            cache: HashMap::new(),
            rng: 0xDEAD_BEEF_CAFE_1234,
        }
    }

    /// Advances every entity's sprite animation by `dt` seconds.
    pub fn update(&mut self, world: &mut World, dt: f32) {
        let ids: Vec<u32> = world
            .iter()
            .filter(|e| !e.sprite.animation.is_empty() && !e.sprite.finished)
            .map(|e| e.id)
            .collect();
        for id in ids {
            let (sprite_name, anim_id) = {
                let e = world.get(id).expect("listed");
                (e.sprite.sprite.clone(), e.sprite.animation.clone())
            };
            let Some(sprite) = self.bank.sprite(&sprite_name) else {
                continue;
            };
            let Some(anim) = sprite.animation(&anim_id) else {
                continue;
            };
            let frames = self.frames(sprite, anim);
            let len = frames.len() as f32;
            if len == 0.0 {
                continue;
            }
            let e = world.get_mut(id).expect("listed");
            if e.sprite.rate != 0.0 {
                if anim.delay > 0.0 {
                    // Normal case: `animationTimer += dt * Rate`, compared
                    // against `Delay` — a fractional frame index accumulates at
                    // `rate * dt / delay`.
                    e.sprite.frame += e.sprite.rate * dt / anim.delay;
                } else if e.sprite.rate > 0.0 {
                    // `delay="0"` in Sprites.xml (single-frame `Loop`s like
                    // `idle`, `duck`, ...): the frame timer condition
                    // `|animationTimer| >= Delay` is immediately true, so the
                    // animation advances one whole frame per tick. Dividing by
                    // zero would produce Infinity → NaN after `% len`.
                    e.sprite.frame += 1.0;
                } else {
                    e.sprite.frame -= 1.0;
                }
            }
            if e.sprite.frame >= len || e.sprite.frame < 0.0 {
                // Monocle guards both ends: `CurrentAnimationFrame < 0 ||
                // CurrentAnimationFrame >= Frames.Length`. The lower bound is
                // reachable because `Rate` may be negative (`Sprite.Reverse`,
                // `sprite.Rate = -1f` in Trapdoor.cs / Player.cs); it used to
                // run the counter off into negative frames here.
                let overshot_low = e.sprite.frame < 0.0;
                if let Some(ref chooser) = anim.goto {
                    let next = chooser.choose(&mut self.rng);
                    e.sprite.animation = next.to_string();
                    e.sprite.finished = false;
                    // C# keeps `animationTimer` and only restarts the frame
                    // index: back to the last frame in reverse, to the first
                    // going forward.
                    e.sprite.frame = wrap_index(e.sprite.frame, len, overshot_low);
                } else if anim.is_loop {
                    e.sprite.frame = wrap_index(e.sprite.frame, len, overshot_low);
                } else {
                    // Non-loop animation finished with no goto.
                    // Pin the last frame and stop advancing, but keep the
                    // animation id so the renderer continues drawing that
                    // frame. Clearing the id would make the entire entity
                    // invisible, unlike Monocle.Sprite. Reversed playback pins
                    // frame 0 instead, matching `CurrentAnimationFrame < 0`.
                    e.sprite.frame = if overshot_low { 0.0 } else { len - 1.0 };
                    e.sprite.finished = true;
                }
            }
        }
    }

    /// The list of atlas frame ids backing an animation.
    pub fn frames(&mut self, sprite: &SpriteData, anim: &Animation) -> Vec<String> {
        let key = (sprite.name.clone(), anim.id.clone());
        if let Some(frames) = self.cache.get(&key) {
            return frames.clone();
        }
        let frames = resolve_frames(self.bank, sprite, anim, self.atlas);
        self.cache.insert(key, frames.clone());
        frames
    }

    /// Resolves the current frame id for an entity, if any.
    pub fn current_frame(
        &mut self,
        sprite: &SpriteData,
        anim: &Animation,
        frame: f32,
    ) -> Option<String> {
        let frames = self.frames(sprite, anim);
        if frames.is_empty() {
            return None;
        }
        // `frame.floor() as usize` saturates every negative value to 0, which
        // would silently draw frame 0 for reversed (`Rate < 0`) playback. Fold
        // the index into range the same way the animator wraps it.
        let len = frames.len() as f32;
        let idx = wrap_index(frame.floor(), len, frame < 0.0);
        frames.get(idx as usize).cloned()
    }
}

/// Resolves the atlas frame ids backing `anim`, without touching the
/// animator's cache.
///
/// The same walk the original does in `Sprite.GetFrames`: an animation with no
/// explicit `frames` takes every consecutive subtexture under its prefix, while
/// one with a `frames` list indexes the prefix directly.
#[must_use]
pub fn resolve_frames(
    _bank: &SpriteBank,
    sprite: &SpriteData,
    anim: &Animation,
    atlas: &Atlas,
) -> Vec<String> {
    let prefix = sprite.texture_prefix(anim);
    if anim.frames.is_empty() {
        (0..subtexture_count(atlas, &prefix))
            .map(|i| subtexture_key(atlas, &prefix, i))
            .collect()
    } else {
        anim.frames
            .iter()
            .map(|&i| subtexture_key(atlas, &prefix, i))
            .collect()
    }
}

/// Returns the number of atlas subtextures for a prefix, discovered the
/// same way `Monocle.Atlas.GetAtlasSubtextureFromAtlasAt` walks them.
fn subtexture_count(atlas: &Atlas, prefix: &str) -> u32 {
    let mut idx = 0u32;
    while subtexture_key_exists(atlas, prefix, idx) {
        idx += 1;
    }
    idx
}

/// The atlas key for the `index`-th subtexture of `prefix`, trying the
/// zero-padded spellings the original `Atlas` uses.
fn subtexture_key(atlas: &Atlas, prefix: &str, index: u32) -> String {
    if index == 0 && atlas.has_frame(prefix) {
        return prefix.to_string();
    }
    let base: String = index.to_string();
    for pad in 0..=6 {
        let width: usize = base.len() + pad;
        let key = format!("{prefix}{:0>width$}", base, width = width);
        if atlas.has_frame(&key) {
            return key;
        }
    }
    format!("{prefix}{index:02}")
}

fn subtexture_key_exists(atlas: &Atlas, prefix: &str, index: u32) -> bool {
    if index == 0 && atlas.has_frame(prefix) {
        return true;
    }
    let base: String = index.to_string();
    (0..=6).any(|pad| {
        let width: usize = base.len() + pad;
        let key = format!("{prefix}{:0>width$}", base, width = width);
        atlas.has_frame(&key)
    })
}

/// Reduces a floating frame index back into `[0, len)` after it stepped past
/// either end of the animation.
///
/// Forward playback uses a plain remainder. Reversed playback has to match
/// Monocle, where the frame index is an *integer* counter that is only
/// advanced by one whole step per tick: stepping back off frame 0 wraps to
/// `len - 1`, not to `len - 1` minus the overshoot.
fn wrap_index(frame: f32, len: f32, overshot_low: bool) -> f32 {
    if overshot_low {
        len - 1.0
    } else {
        frame.rem_euclid(len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::spritebank::{Animation, SpriteData};
    use crate::engine::ecs::World;

    /// Builds a bare `Atlas` with the given frame ids present (empty pixel
    /// pages; enough for `has_frame` / animation resolution).
    fn atlas_with_frames(ids: &[&str]) -> Atlas {
        let mut atlas = Atlas::default();
        for (i, id) in ids.iter().enumerate() {
            atlas.frame_index.insert(id.to_string(), (0, i));
        }
        atlas
    }

    /// A tiny sprite with one loop animation whose frames map to atlas ids
    /// `test/idle00`, `test/idle01`, ...
    fn loop_sprite() -> (SpriteData, Animation) {
        let sprite = SpriteData {
            name: "tb".to_string(),
            path: "test/".to_string(),
            start: "idle".to_string(),
            origin: (0, 0),
            center: false,
            justify: None,
            animations: HashMap::new(),
        };
        let anim = Animation {
            id: "idle".to_string(),
            path: "idle".to_string(),
            delay: 0.1,
            frames: Vec::new(),
            goto: None,
            is_loop: true,
        };
        (sprite, anim)
    }

    #[test]
    fn loop_animation_converges_large_randomized_frame() {
        let atlas = atlas_with_frames(&[
            "test/idle00",
            "test/idle01",
            "test/idle02",
            "test/idle03",
            "test/idle04",
        ]);
        let (mut sprite, anim) = loop_sprite();
        sprite.animations.insert(anim.id.clone(), anim.clone());
        let mut bank = SpriteBank::default();
        bank.sprites.insert(sprite.name.clone(), sprite.clone());

        let mut world = World::default();
        let id = world.spawn();
        {
            let e = world.get_mut(id).unwrap();
            e.sprite.sprite = "tb".to_string();
            e.sprite.animation = "idle".to_string();
            // Simulate `randomizeFrame`: a large random starting frame.
            e.sprite.frame = 60_007.0;
        }

        let mut animator = SpriteAnimator::new(&atlas, &bank);
        // Advance one tick; `rate * dt / delay` with rate=1, dt=1/60, delay=0.1
        // adds ~0.167, pushing frame far past len=4.
        animator.update(&mut world, 1.0 / 60.0);
        let e = world.get(id).unwrap();
        // The loop's `frame %= len` must have reduced it into [0, 4).
        assert!(
            (0.0..4.0).contains(&e.sprite.frame),
            "loop animation frame {} not reduced into [0,4)",
            e.sprite.frame
        );
        // And it must not be exactly 0 (the randomized phase survived).
        assert!(
            (0.0..4.0).contains(&e.sprite.frame) && e.sprite.frame != 0.0,
            "randomized loop phase lost"
        );
    }

    #[test]
    fn non_loop_animation_keeps_final_frame_on_finish() {
        let atlas = atlas_with_frames(&["test/once00", "test/once01"]);
        let sprite = SpriteData {
            name: "tb".to_string(),
            path: "test/".to_string(),
            start: "once".to_string(),
            origin: (0, 0),
            center: false,
            justify: None,
            animations: HashMap::new(),
        };
        let anim = Animation {
            id: "once".to_string(),
            path: "once".to_string(),
            delay: 0.1,
            frames: Vec::new(),
            goto: None,
            is_loop: false,
        };
        let mut sprite = sprite;
        sprite.animations.insert(anim.id.clone(), anim.clone());
        let mut bank = SpriteBank::default();
        bank.sprites.insert(sprite.name.clone(), sprite.clone());

        let mut world = World::default();
        let id = world.spawn();
        {
            let e = world.get_mut(id).unwrap();
            e.sprite.sprite = "tb".to_string();
            e.sprite.animation = "once".to_string();
            e.sprite.frame = 0.0;
        }
        let mut animator = SpriteAnimator::new(&atlas, &bank);
        // Push well past the 2-frame end.
        animator.update(&mut world, 1.0);
        let e = world.get(id).unwrap();
        // The final frame remains active and renderable after the one-shot ends.
        assert_eq!(e.sprite.animation, "once");
        assert!(e.sprite.finished, "non-loop should stop advancing");
        assert_eq!(e.sprite.frame, 1.0, "frame pinned to last frame");

        // A later tick must not advance or clear the final frame.
        animator.update(&mut world, 1.0);
        let e = world.get(id).unwrap();
        assert_eq!(e.sprite.animation, "once");
        assert!(e.sprite.finished);
        assert_eq!(e.sprite.frame, 1.0);
    }

    #[test]
    fn current_frame_wraps_negative_indices_instead_of_pinning_to_zero() {
        let atlas = atlas_with_frames(&[
            "test/idle00",
            "test/idle01",
            "test/idle02",
            "test/idle03",
            "test/idle04",
        ]);
        let (mut sprite, anim) = loop_sprite();
        sprite.animations.insert(anim.id.clone(), anim.clone());
        let mut bank = SpriteBank::default();
        bank.sprites.insert(sprite.name.clone(), sprite.clone());
        let mut animator = SpriteAnimator::new(&atlas, &bank);

        // Forward indices clamp/resolve normally.
        assert_eq!(
            animator.current_frame(&sprite, &anim, 0.0).as_deref(),
            Some("test/idle00")
        );
        assert_eq!(
            animator.current_frame(&sprite, &anim, 3.9).as_deref(),
            Some("test/idle03")
        );
        // Reversed playback holds a negative fractional index; the old
        // `frame.floor() as usize` saturated it to frame 0. The animator only
        // ever hands out a negative index on the tick that crosses the low
        // bound, and that tick pins the frame, so the last frame is the only
        // value a renderer can legitimately observe here.
        assert_eq!(
            animator.current_frame(&sprite, &anim, -1.0).as_deref(),
            Some("test/idle04")
        );
        assert_eq!(
            animator.current_frame(&sprite, &anim, -3.5).as_deref(),
            Some("test/idle04")
        );
    }

    #[test]
    fn reverse_rate_wraps_loop_back_to_the_first_frame() {
        let atlas = atlas_with_frames(&[
            "test/idle00",
            "test/idle01",
            "test/idle02",
            "test/idle03",
            "test/idle04",
        ]);
        let (mut sprite, anim) = loop_sprite();
        sprite.animations.insert(anim.id.clone(), anim.clone());
        let mut bank = SpriteBank::default();
        bank.sprites.insert(sprite.name.clone(), sprite.clone());

        let mut world = World::default();
        let id = world.spawn();
        {
            let e = world.get_mut(id).unwrap();
            e.sprite.sprite = "tb".to_string();
            e.sprite.animation = "idle".to_string();
            e.sprite.frame = 0.0;
            // `Sprite.Reverse` / `sprite.Rate = -1f` (Trapdoor.cs, Player.cs).
            e.sprite.rate = -1.0;
        }

        let mut animator = SpriteAnimator::new(&atlas, &bank);
        // Five ticks at rate -1 and delay 0.1 walk the 5-frame loop backwards
        // off the front. Monocle handles `CurrentAnimationFrame < 0` in the same
        // branch that handles the upper bound, wrapping to `len - 1`.
        for _ in 0..5 {
            animator.update(&mut world, 0.1);
        }
        let e = world.get(id).unwrap();
        assert!(
            (0.0..5.0).contains(&e.sprite.frame),
            "reversed loop frame {} escaped [0,5)",
            e.sprite.frame
        );
    }

    #[test]
    fn reverse_rate_finishes_a_one_shot_at_frame_zero() {
        let atlas = atlas_with_frames(&["test/once00", "test/once01"]);
        let sprite = SpriteData {
            name: "tb".to_string(),
            path: "test/".to_string(),
            start: "once".to_string(),
            origin: (0, 0),
            center: false,
            justify: None,
            animations: HashMap::new(),
        };
        let anim = Animation {
            id: "once".to_string(),
            path: "once".to_string(),
            delay: 0.1,
            frames: Vec::new(),
            goto: None,
            is_loop: false,
        };
        let mut sprite = sprite;
        sprite.animations.insert(anim.id.clone(), anim.clone());
        let mut bank = SpriteBank::default();
        bank.sprites.insert(sprite.name.clone(), sprite.clone());

        let mut world = World::default();
        let id = world.spawn();
        {
            let e = world.get_mut(id).unwrap();
            e.sprite.sprite = "tb".to_string();
            e.sprite.animation = "once".to_string();
            e.sprite.frame = 1.0;
            e.sprite.rate = -1.0;
        }

        let mut animator = SpriteAnimator::new(&atlas, &bank);
        // One tick walks 1 -> 0 (still in range). The next tick walks 0 -> -1,
        // crossing the low bound, which finishes the animation; Monocle then
        // pins it to frame 0 because `CurrentAnimationFrame < 0`.
        animator.update(&mut world, 0.1);
        assert_eq!(world.get(id).unwrap().sprite.frame, 0.0);
        assert!(!world.get(id).unwrap().sprite.finished);
        animator.update(&mut world, 0.1);
        let e = world.get(id).unwrap();
        assert!(e.sprite.finished, "reversed one-shot should stop advancing");
        assert_eq!(e.sprite.frame, 0.0, "reversed one-shot pins to frame 0");
    }

    #[test]
    fn zero_delay_loop_does_not_produce_nan() {
        // `Sprites.xml` has many `delay="0"` single-frame Loop animations
        // (player `idle`, `duck`, `pretendDead`, ...). The original advances
        // one full frame per tick (`|animationTimer| >= Delay` is immediately
        // true); dividing by zero used to yield Infinity → NaN on `% len`.
        let atlas = atlas_with_frames(&["test/zero00"]);
        let sprite = SpriteData {
            name: "tb".to_string(),
            path: "test/".to_string(),
            start: "zero".to_string(),
            origin: (0, 0),
            center: false,
            justify: None,
            animations: HashMap::new(),
        };
        let anim = Animation {
            id: "zero".to_string(),
            path: "zero".to_string(),
            delay: 0.0,
            frames: Vec::new(),
            goto: None,
            is_loop: true,
        };
        let mut sprite = sprite;
        sprite.animations.insert(anim.id.clone(), anim.clone());
        let mut bank = SpriteBank::default();
        bank.sprites.insert(sprite.name.clone(), sprite.clone());

        let mut world = World::default();
        let id = world.spawn();
        {
            let e = world.get_mut(id).unwrap();
            e.sprite.sprite = "tb".to_string();
            e.sprite.animation = "zero".to_string();
            e.sprite.frame = 0.0;
        }
        let mut animator = SpriteAnimator::new(&atlas, &bank);
        for _ in 0..120 {
            animator.update(&mut world, 1.0 / 60.0);
        }
        let e = world.get(id).unwrap();
        assert!(
            e.sprite.frame.is_finite(),
            "single-frame loop frame must stay finite, got {}",
            e.sprite.frame
        );
        assert_eq!(e.sprite.frame, 0.0, "single-frame loop pins to frame 0");
    }
}
