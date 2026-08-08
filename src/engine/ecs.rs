//! A minimal entity registry for the host. All gameplay lives in Wasm
//! plugins; this registry only stores the shared state that plugins operate on
//! through the FFI (position, speed, sprite state, ...).

use std::collections::{HashMap, HashSet};

use ruleste_plugin_api::types::{Color, Vec2};

#[derive(Debug, Clone)]
pub struct SpriteState {
    /// Sprite bank entry, e.g. "player".
    pub sprite: String,
    pub animation: String,
    pub frame: f32,
    pub rate: f32,
    pub color: Color,
    pub flip_x: bool,
    pub flip_y: bool,
}

impl Default for SpriteState {
    fn default() -> SpriteState {
        SpriteState {
            sprite: String::new(),
            animation: String::new(),
            frame: 0.0,
            rate: 1.0,
            color: Color::WHITE,
            flip_x: false,
            flip_y: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub id: u32,
    /// Name of the plugin that owns this entity (empty = unmanaged).
    pub plugin: String,
    /// Map entity type, e.g. "player".
    pub entity_type: String,
    /// Serialized spawn data (`MapData::to_bytes`).
    pub spawn: Vec<u8>,
    pub position: Vec2,
    pub speed: Vec2,
    /// Collision rect = `position + hitbox_offset`, size `hitbox`.
    pub hitbox: Vec2,
    pub hitbox_offset: Vec2,
    pub depth: i32,
    pub visible: bool,
    pub sprite: SpriteState,
}

impl Entity {
    pub fn new(id: u32) -> Entity {
        Entity {
            id,
            plugin: String::new(),
            entity_type: String::new(),
            spawn: Vec::new(),
            position: Vec2::ZERO,
            speed: Vec2::ZERO,
            hitbox: Vec2::new(8.0, 11.0),
            hitbox_offset: Vec2::ZERO,
            depth: 0,
            visible: true,
            sprite: SpriteState::default(),
        }
    }
}

#[derive(Default)]
pub struct World {
    entities: HashMap<u32, Entity>,
    next_id: u32,
    /// Entities acting as standable platforms (dynamic solids). Their top
    /// surface catches falling actors like a jump-thru, and their movement
    /// carries along any actor standing on them.
    pub solid_platforms: HashSet<u32>,
}

impl World {
    pub fn new() -> World {
        World::default()
    }

    pub fn spawn(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.entities.insert(id, Entity::new(id));
        id
    }

    pub fn despawn(&mut self, id: u32) {
        self.entities.remove(&id);
    }

    pub fn get(&self, id: u32) -> Option<&Entity> {
        self.entities.get(&id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut Entity> {
        self.entities.get_mut(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.entities.values()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Entity> {
        self.entities.values_mut()
    }

    pub fn entity_ids(&self) -> Vec<u32> {
        self.entities.keys().copied().collect()
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn is_alive(&self, id: u32) -> bool {
        self.entities.contains_key(&id)
    }
}
