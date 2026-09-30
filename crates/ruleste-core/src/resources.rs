//! Explicit resource-need manifests.
//!
//! The client renderer consumes these to load *only* the textures and audio
//! the current set of entities actually references, instead of uploading every
//! atlas page up front. The core computes the manifest from the map's entity
//! spawns, the SpriteBank definitions and the autotiler output — no rendering
//! knowledge lives here, so this module is a pure "what does the game need"
//! query surface for the client.

use std::collections::HashSet;

use crate::data::atlas::Atlas;
use crate::data::spritebank::SpriteBank;
use crate::engine::autotiler::TileGrid;

/// Everything the client should load to render a given set of entity types.
#[derive(Debug, Clone, Default)]
pub struct ResourceManifest {
    /// Atlas frame ids referenced by the entity types' SpriteBank animations
    /// (already resolved against the atlas, so aliases like `player_no_backpack`
    /// and case differences are folded into the concrete frame ids).
    pub frames: HashSet<String>,
    /// Atlas frames used as parallax backdrop textures.
    pub backdrops: HashSet<String>,
    /// Tileset texture paths used by the tile grids (per-tile `tileset` field
    /// of a `TileGrid`, e.g. `"template"` or `"dirt"`).
    pub tilesets: HashSet<String>,
    /// Audio event/bank names the entities may play (best-effort; static
    /// knowledge only).
    pub audio: HashSet<String>,
}

impl ResourceManifest {
    /// Builds the manifest for a set of live entity types.
    #[must_use]
    pub fn for_entity_types(
        entity_types: &[String],
        atlas: &Atlas,
        bank: &SpriteBank,
    ) -> ResourceManifest {
        let mut manifest = ResourceManifest::default();
        for ty in entity_types {
            for frame in sprite_frames_for(atlas, bank, ty) {
                manifest.frames.insert(frame);
            }
        }
        manifest
    }

    /// Adds every frame referenced by a single SpriteBank sprite, resolved
    /// against the atlas (missing entries are dropped).
    #[must_use]
    pub fn with_sprite(mut self, atlas: &Atlas, bank: &SpriteBank, name: &str) -> Self {
        self.frames.extend(sprite_frames_for(atlas, bank, name));
        self
    }

    /// Records the tileset paths used by a tile grid (e.g. the active room's
    /// solid and background grids).
    pub fn add_tile_grid(&mut self, grid: &TileGrid) {
        self.tilesets
            .extend(grid.tileset.iter().filter(|p| !p.is_empty()).cloned());
    }

    /// Records an audio event name the client should preload.
    pub fn add_audio(&mut self, name: impl Into<String>) {
        self.audio.insert(name.into());
    }
}

/// Expands every animation of a sprite into concrete atlas frame ids, using the
/// same zero-padded subtexture walk the animator uses at runtime. Frame ids that
/// don't resolve in the atlas are skipped.
fn sprite_frames_for(atlas: &Atlas, bank: &SpriteBank, name: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let Some(sprite) = bank.sprite(name) else {
        return out;
    };
    for anim in sprite.animations.values() {
        let prefix = sprite.texture_prefix(anim);
        if anim.frames.is_empty() {
            // Walk `prefix`, `prefix0`, `prefix00`, ... like the animator.
            for index in 0..atlas_subtexture_count(atlas, &prefix) {
                if let Some(id) = atlas_subtexture_key(atlas, &prefix, index) {
                    out.insert(id);
                }
            }
        } else {
            for &frame in &anim.frames {
                if let Some(id) = atlas_subtexture_key(atlas, &prefix, frame) {
                    out.insert(id);
                }
            }
        }
    }
    out
}

/// Count of consecutive atlas subtextures under a prefix (mirrors
/// `SpriteAnimator::subtexture_count`).
fn atlas_subtexture_count(atlas: &Atlas, prefix: &str) -> u32 {
    let mut idx = 0;
    while atlas_subtexture_key(atlas, prefix, idx).is_some() {
        idx += 1;
    }
    idx
}

/// The atlas frame id for `prefix` + zero-padded `index`, or `None` when no
/// such frame exists (mirrors `SpriteAnimator::subtexture_key`).
fn atlas_subtexture_key(atlas: &Atlas, prefix: &str, index: u32) -> Option<String> {
    if index == 0 && atlas.has_frame(prefix) {
        return Some(prefix.to_string());
    }
    let base = index.to_string();
    for pad in 0..=6 {
        let key = format!("{prefix}{base:0>width$}", width = base.len() + pad);
        if atlas.has_frame(&key) {
            return Some(key);
        }
    }
    None
}

/// Returns the number of distinct atlas frames the manifest references — a
/// cheap way for the client to gauge how much texture it will actually upload.
#[must_use]
pub fn manifest_frame_count(manifest: &ResourceManifest) -> usize {
    manifest.frames.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::spritebank::Animation;

    fn atlas_with(ids: &[&str]) -> Atlas {
        let mut atlas = Atlas::default();
        for (i, id) in ids.iter().enumerate() {
            atlas.frame_index.insert(id.to_string(), (0, i));
        }
        atlas
    }

    #[test]
    fn manifest_expands_sprite_anim_frames() {
        let atlas = atlas_with(&["char/run00", "char/run01", "char/run02", "char/idle00"]);
        let mut bank = SpriteBank::default();
        let mut sprite = crate::data::spritebank::SpriteData {
            name: "p".into(),
            path: "char/".into(),
            start: "idle".into(),
            origin: (0, 0),
            center: false,
            justify: None,
            animations: Default::default(),
        };
        sprite.animations.insert(
            "run".into(),
            Animation {
                id: "run".into(),
                path: "run".into(),
                delay: 0.1,
                frames: Vec::new(),
                goto: None,
                is_loop: true,
            },
        );
        sprite.animations.insert(
            "idle".into(),
            Animation {
                id: "idle".into(),
                path: "idle".into(),
                delay: 0.0,
                frames: Vec::new(),
                goto: None,
                is_loop: true,
            },
        );
        bank.sprites.insert("p".into(), sprite);

        let m = ResourceManifest::for_entity_types(&["p".to_string()], &atlas, &bank);
        // run00..02 + idle00 (all present in the atlas).
        assert_eq!(m.frames.len(), 4, "frames: {:?}", m.frames);
        assert!(m.frames.contains("char/run00"));
        assert!(m.frames.contains("char/run01"));
        assert!(m.frames.contains("char/run02"));
        assert!(m.frames.contains("char/idle00"));
    }

    #[test]
    fn manifest_tile_grid_records_tilesets() {
        let mut m = ResourceManifest::default();
        let grid = TileGrid {
            width: 2,
            height: 1,
            tileset: vec!["dirt".into(), "".into()],
            col: vec![0, 0],
            row: vec![0, 0],
            origin_x: 0.0,
            origin_y: 0.0,
        };
        m.add_tile_grid(&grid);
        assert_eq!(m.tilesets, HashSet::from(["dirt".to_string()]));
    }
}
