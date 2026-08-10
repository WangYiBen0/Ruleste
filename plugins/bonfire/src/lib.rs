#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `bonfire` entity plugin.
//!
//! Mirrors the `Bonfire` campfire found in the Old Site: a log pile whose
//! flame flickers on a looping clock. Ember particles need an emitter system
//! the host does not have, so the fire is drawn procedurally as stacked
//! flame rectangles whose height and hue pulse with a sine.

use ruleste_plugin_api::host::draw_rect;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("bonfire");
ruleste_plugin_api::ruleste_entity_types!("bonfire");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

const LOG_COLOR: Color = Color {
    r: 0x5a,
    g: 0x2d,
    b: 0x18,
    a: 0xff,
};

#[derive(Debug, Default)]
struct FireState {
    timer: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<FireState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut FireState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, FireState::default) as *mut FireState;
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
    entity.depth.set(2000);
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
        // Log pile.
        draw_rect(p.x - 10.0, p.y - 2.0, 20.0, 4.0, LOG_COLOR);
        // Flickering flame: two layered triangles approximated by stacked
        // rects whose heights follow independent sines.
        let flicker = 1.0 + (st.timer * 9.0).sin() * 0.15;
        let sway = (st.timer * 7.0).sin() * 1.5;
        let outer_f = 6.0 * flicker;
        let inner_f = 4.0 * (1.0 + (st.timer * 12.0 + 1.3).sin() * 0.2);
        let core = Color {
            r: 0xff,
            g: 0xe0,
            b: 0x60,
            a: 0xf0,
        };
        let mid = Color {
            r: 0xe0,
            g: 0x80,
            b: 0x20,
            a: 0xe8,
        };
        let outer = Color {
            r: 0x90,
            g: 0x30,
            b: 0x10,
            a: 0xc0,
        };
        draw_rect(p.x - 5.0, p.y - inner_f, 10.0, inner_f, core);
        draw_rect(p.x - 8.0 + sway, p.y - outer_f, 16.0, outer_f, mid);
        draw_rect(
            p.x - 10.0 + sway * 0.5,
            p.y - outer_f * 1.3,
            20.0,
            outer_f * 1.3,
            outer,
        );
    });
}
