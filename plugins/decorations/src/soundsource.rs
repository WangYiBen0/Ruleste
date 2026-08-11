//! `soundSource` entity plugin.
//!
//! Mirrors `SoundSourceEntity.cs`: a depth -8500, invisible entity that plays
//! an FMOD event (`data.Attr("sound")`) on spawn. The engine has no audio
//! backend yet, so the plugin only remembers the event name for the future
//! SDL audio integration and does nothing else.

use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::EntityId;

#[derive(Debug, Default)]
struct SoundState {
    /// `data.Attr("sound")` — FMOD event handle, unused until audio lands.
    event_name: String,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<SoundState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut SoundState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, SoundState::default) as *mut SoundState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

pub fn init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.depth.set(-8500);
    with_state(id, |st| {
        st.event_name = spawn.get_str("sound", "");
    });
}

pub fn update(_id: EntityId, _dt: f32) {}

pub fn draw(_id: EntityId) {}
