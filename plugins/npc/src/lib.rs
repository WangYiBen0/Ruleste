#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `npc` entity plugin.
//!
//! Mirrors `NPC.cs` (granny, snowman, ...): a non-interactive character that
//! stands on a floor marker. Dialogue exists only as far as a spoken quote
//! needs the host's text system (not implemented), so the plugin draws the
//! character as a simple silhouette — palette picked by the `npc` id — with a
//! tiny idle bob.

use ruleste_plugin_api::host::draw_rect;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("npc");
ruleste_plugin_api::ruleste_entity_types!("npc");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

const SKIN: Color = Color {
    r: 0x9a,
    g: 0x84,
    b: 0x70,
    a: 0xff,
};
const COAT: Color = Color {
    r: 0x38,
    g: 0x40,
    b: 0x50,
    a: 0xff,
};
const SCARF: Color = Color {
    r: 0xc0,
    g: 0x40,
    b: 0x40,
    a: 0xff,
};

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
    let _ = spawn.get_str("npc", "");
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.timer += dt;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let p = Entity::new(id).position.get();
        // Granny silhouette standing on the ground marker: coat, head, scarf.
        let bob = (st.timer * 2.0).sin().abs() * 1.0;
        let (hx, hy) = (p.x, p.y + bob);
        draw_rect(hx - 5.0, hy - 26.0, 10.0, 26.0, COAT);
        draw_rect(hx - 3.0, hy - 30.0, 6.0, 6.0, SKIN);
        draw_rect(hx - 3.0, hy - 22.0, 6.0, 3.0, SCARF);
        draw_rect(
            hx - 4.0,
            hy - 26.0,
            8.0,
            2.0,
            Color::new(0x88, 0x90, 0x9a, 0xff),
        );
    });
}
