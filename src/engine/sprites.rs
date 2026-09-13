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
            .filter(|e| !e.sprite.animation.is_empty())
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
            if e.sprite.frame >= len {
                if let Some(ref chooser) = anim.goto {
                    let next = chooser.choose(&mut self.rng);
                    e.sprite.animation = next.to_string();
                    e.sprite.frame = 0.0;
                } else if anim.is_loop {
                    e.sprite.frame %= len;
                } else {
                    // Non-loop animation finished with no goto.
                    // Pin to last frame and clear animation (equivalent to
                    // C# `Animating = false` + clearing state).
                    e.sprite.frame = len - 1.0;
                    e.sprite.animation.clear();
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
        let prefix = sprite.texture_prefix(anim);
        let count = self.subtexture_count(&prefix);
        let frames: Vec<String> = if anim.frames.is_empty() {
            (0..count)
                .map(|i| self.subtexture_key(&prefix, i))
                .collect()
        } else {
            anim.frames
                .iter()
                .map(|&i| self.subtexture_key(&prefix, i))
                .collect()
        };
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
        let idx = (frame.floor() as usize).min(frames.len() - 1);
        frames.get(idx).cloned()
    }

    /// Returns the number of atlas subtextures for a prefix, discovered the
    /// same way `Monocle.Atlas.GetAtlasSubtextureFromAtlasAt` walks them.
    fn subtexture_count(&self, prefix: &str) -> u32 {
        let mut idx = 0u32;
        while self.subtexture_key_exists(prefix, idx) {
            idx += 1;
        }
        idx
    }

    fn subtexture_key(&self, prefix: &str, index: u32) -> String {
        if index == 0 && self.atlas.has_frame(prefix) {
            return prefix.to_string();
        }
        let base: String = index.to_string();
        for pad in 0..=6 {
            let width: usize = base.len() + pad;
            let key = format!("{prefix}{:0>width$}", base, width = width);
            if self.atlas.has_frame(&key) {
                return key;
            }
        }
        format!("{prefix}{index:02}")
    }

    fn subtexture_key_exists(&self, prefix: &str, index: u32) -> bool {
        if index == 0 && self.atlas.has_frame(prefix) {
            return true;
        }
        let base: String = index.to_string();
        (0..=6).any(|pad| {
            let width: usize = base.len() + pad;
            let key = format!("{prefix}{:0>width$}", base, width = width);
            self.atlas.has_frame(&key)
        })
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
    fn non_loop_animation_clears_animation_on_finish() {
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
        // Mirrors C# `Animating = false` + clearing `AnimationID`.
        assert!(
            e.sprite.animation.is_empty(),
            "non-loop should clear animation"
        );
        assert_eq!(e.sprite.frame, 1.0, "frame pinned to last frame");
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
