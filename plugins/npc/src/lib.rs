#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `npc` entity plugin.
//!
//! Mirrors `NPC.cs` (granny, snowman, ...): a non-interactive character that
//! stands on a floor marker. Dialogue exists only as far as a spoken quote
//! needs the host's text system (not implemented), so the plugin draws the
//! character as a simple silhouette — palette picked by the `npc` id — with a
//! tiny idle bob.

use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::EntityId;

ruleste_plugins_api::ruleste_meta!("npc");
ruleste_plugins_api::ruleste_entity_types!("npc");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

#[derive(Debug, Default)]
struct NpcState {
    timer: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<NpcState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut NpcState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, NpcState::default) as *mut NpcState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity = Entity::new(id);
    entity
        .position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    entity.depth.set(-2000);
    // Render the character with the real `player` SpriteBank sprite (idle pose).
    entity.sprite.set_bank("player");
    entity.sprite.play("idle");
    let _ = spawn.get_str("npc", "");
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.timer += dt;
    });
}

// The NPC is drawn by the host SpriteBank renderer via `entity.sprite`.
#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(_id: EntityId) {}
