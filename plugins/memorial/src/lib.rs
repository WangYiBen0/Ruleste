#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `memorial` entity plugin.
//!
//! Mirrors `Memorial.cs`: a commemorative stone on the mountain paths. In the
//! original, standing next to the slab fades the screen and shows the
//! memorial quote; the engine has no dialogue/overlay system yet, so this
//! plugin only tracks proximity (for later UI) and draws the slab itself as a
//! procedural stone. The sprite frames are looked up when available and fall
//! back to the drawn stone when the atlas lacks them.

use ruleste_plugins_api::host::{self, draw_image, draw_rect, entities_by_type};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

ruleste_plugins_api::ruleste_meta!("memorial");
ruleste_plugins_api::ruleste_entity_types!("memorial");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const STONE: Color = Color {
    r: 0x9c,
    g: 0xa4,
    b: 0xae,
    a: 0xff,
};
const STONE_TOP: Color = Color {
    r: 0xc8,
    g: 0xd0,
    b: 0xd8,
    a: 0xff,
};
/// Proximity radius for the (not yet wired-up) memorial face-in.
const RADIUS: f32 = 16.0;

#[derive(Debug, Default)]
struct MemorialState {
    touched: bool,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<MemorialState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut MemorialState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, MemorialState::default) as *mut MemorialState;
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
    entity.depth.set(0);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, _dt: f32) {
    with_state(id, |st| {
        if st.touched {
            return;
        }
        let p = Entity::new(id).position.get();
        for player_id in entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let dx = pp.x - p.x;
            let dy = pp.y - p.y;
            if dx * dx + dy * dy <= RADIUS * RADIUS {
                st.touched = true;
            }
            break;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let entity = Entity::new(id);
    let p = entity.position.get();
    // Prefer the real atlas art when the pack provides it, otherwise the slab.
    draw_image("objects/memorial/frame/memorial00", p.x, p.y, 0.0, 1.0, 1.0);
    // Slab body.
    draw_rect(p.x - 10.0, p.y - 4.0, 20.0, 14.0, STONE);
    draw_rect(p.x - 12.0, p.y - 8.0, 24.0, 5.0, STONE_TOP);
}
