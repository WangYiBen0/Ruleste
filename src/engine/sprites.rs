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
                e.sprite.frame += e.sprite.rate * dt / anim.delay;
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
