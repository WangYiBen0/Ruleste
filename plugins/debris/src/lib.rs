#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `floatingDebris` / `foregroundDebris` entity plugins.
//!
//! The original textures (`scenery/debris`, `scenery/fgdebris/rock_a|b`) live
//! in the per-chapter OldSite atlas, which the host doesn't load yet, so both
//! are rendered as simple procedural rubble until multi-atlas support lands.
//! `floatingDebris` bobs and spins slowly; `foregroundDebris` is a far
//! background parallax stone.

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("debris");
ruleste_plugin_api::ruleste_entity_types!("floatingDebris", "foregroundDebris");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

#[derive(Clone, Copy, Debug)]
struct DebrisState {
    start_y: f32,
    timer: f32,
    foreground: bool,
}

impl Default for DebrisState {
    fn default() -> DebrisState {
        DebrisState {
            start_y: 0.0,
            timer: 0.0,
            foreground: false,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<DebrisState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut DebrisState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, DebrisState::default) as *mut DebrisState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    let foreground = spawn.get_str("_entity_type", "floatingDebris") == "foregroundDebris";
    entity.depth.set(if foreground { -999900 } else { -5 });
    with_state(id, |st| {
        st.start_y = y;
        st.foreground = foreground;
    });
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
        let entity = Entity::new(id);
        let p = entity.position.get();
        if st.foreground {
            // Dark background stone, mildly shaded.
            host::draw_rect(p.x - 3.0, p.y - 3.0, 6.0, 6.0, Color::new(40, 44, 48, 220));
            host::draw_rect(p.x - 1.5, p.y - 1.5, 3.0, 3.0, Color::new(24, 26, 30, 200));
        } else {
            // Bobbing rubble, ±2 px like the original SineWave.
            let bob = (st.timer * 2.0).sin() * 2.0;
            host::draw_rect(
                p.x - 2.0,
                p.y - 2.0 + bob,
                4.0,
                4.0,
                Color::new(120, 126, 132, 230),
            );
        }
        let _ = st.start_y;
    });
}
