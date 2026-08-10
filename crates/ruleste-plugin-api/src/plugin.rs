//! Helpers for writing Wasm entity plugins: per-entity state storage and the
//! exports every plugin must provide.

use std::vec::Vec;

use crate::{host, map::MapData, types::EntityId};
pub use host::{Collision, Depth, Hitbox, Input, Position, Speed, Sprite};

/// Per-entity plugin state, stored in Wasm linear memory and keyed by entity
/// id. Values are kept across hot reloads via `serialize_state`/`restore_state`.
#[derive(Debug)]
pub struct EntityState<T> {
    items: Vec<(EntityId, T)>,
}

impl<T> Default for EntityState<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T> EntityState<T> {
    pub fn new() -> EntityState<T> {
        EntityState::default()
    }

    #[must_use]
    pub fn get(&self, id: EntityId) -> Option<&T> {
        self.items.iter().find(|(e, _)| *e == id).map(|(_, s)| s)
    }

    #[must_use]
    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut T> {
        self.items
            .iter_mut()
            .find(|(e, _)| *e == id)
            .map(|(_, s)| s)
    }

    pub fn insert(&mut self, id: EntityId, state: T) {
        self.remove(id);
        self.items.push((id, state));
    }

    pub fn get_or_insert(&mut self, id: EntityId, make: impl FnOnce() -> T) -> &mut T {
        if !self.items.iter().any(|(e, _)| *e == id) {
            self.items.push((id, make()));
        }
        self.get_mut(id).expect("state just inserted")
    }

    pub fn remove(&mut self, id: EntityId) {
        self.items.retain(|(e, _)| *e != id);
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

/// The plugin-side representation of an entity the plugin is in charge of.
pub struct Entity {
    pub id: EntityId,
    pub position: Position,
    pub speed: Speed,
    pub sprite: Sprite,
    pub collision: Collision,
    pub hitbox: Hitbox,
    pub depth: Depth,
}

impl Entity {
    pub fn new(id: EntityId) -> Entity {
        Entity {
            id,
            position: Position::new(id),
            speed: Speed::new(id),
            sprite: Sprite::new(id),
            collision: Collision::new(id),
            hitbox: Hitbox::new(id),
            depth: Depth::new(id),
        }
    }
}

/// Reads spawn data passed to `ruleste_entity_init`.
///
/// # Panics
///
/// Panics if the spawn data is malformed; this is a host bug.
pub fn spawn_data(bytes: &[u8]) -> MapData {
    match MapData::from_bytes(bytes) {
        Ok(data) => data,
        Err(err) => panic!("ruleste: malformed spawn data: {err:?}"),
    }
}

/// Scratch-buffer allocator exported to the host. The host reserves a buffer
/// via [`ruleste_alloc`], writes spawn/state bytes into it, calls the plugin,
/// then releases it with [`ruleste_dealloc`]. Uses the module's own allocator
/// so the host never guesses at the heap layout.
#[unsafe(no_mangle)]
pub extern "C" fn ruleste_alloc(len: u32) -> u32 {
    let layout = std::alloc::Layout::from_size_align(len as usize, 8).unwrap();
    unsafe { std::alloc::alloc(layout) as u32 }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_dealloc(ptr: u32, len: u32) {
    let layout = std::alloc::Layout::from_size_align(len as usize, 8).unwrap();
    unsafe { std::alloc::dealloc(ptr as *mut u8, layout) }
}

/// Describes the fixed entry points a plugin module must export.
pub use crate::{event, export};

/// Convenience stubs that satisfy the host runtime's expectations for plugins
/// that don't implement every optional stage.
#[macro_export]
macro_rules! ruleste_meta {
    ($name:literal) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn ruleste_plugin_meta() -> u32 {
            static NAME: &[u8] = concat!($name, "\0").as_bytes();
            NAME.as_ptr() as u32
        }
    };
}

#[macro_export]
macro_rules! ruleste_entity_types {
    ($($ty:literal),+ $(,)?) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn ruleste_plugin_entity_types(out_len: *mut u32) -> u32 {
            static TYPES: &[u8] = concat!($($ty, ",",)+ "\0").as_bytes();
            unsafe { *out_len = (TYPES.len() - 1) as u32 }
            TYPES.as_ptr() as u32
        }
    };
}

/// A do-nothing `ruleste_entity_destroy` so plugins can omit it.
#[macro_export]
macro_rules! ruleste_noop_destroy {
    () => {
        #[unsafe(no_mangle)]
        pub extern "C" fn ruleste_entity_destroy(_id: EntityId) {}
    };
}

/// A do-nothing serializer pair (returns 0, meaning "no state to restore").
#[macro_export]
macro_rules! ruleste_noop_serialize {
    () => {
        #[unsafe(no_mangle)]
        pub extern "C" fn ruleste_entity_serialize(_id: EntityId, out_len: *mut u32) -> u32 {
            unsafe { *out_len = 0 }
            0
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn ruleste_entity_deserialize(_id: EntityId, _data: *const u8, _len: u32) {}
    };
}
