#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `introCar` entity plugin.
//!
//! Mirrors `IntroCar.cs`: the crashed car in prologue and chapter 9.
//! Inherits from `JumpThru` (depth 1) with a standable top platform
//! (`entity.collision.platform(true)`). Sinks slightly when ridden.

use ruleste_plugins_api::host::{self, draw_image};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::EntityId;

ruleste_plugins_api::ruleste_meta!("intro-car");
ruleste_plugins_api::ruleste_entity_types!("introCar");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

#[derive(Debug, Default)]
struct CarState {
    timer: f32,
    start_y: f32,
    sink: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<CarState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut CarState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, CarState::default) as *mut CarState;
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
    // Standable roof platform (JumpThru)
    entity.hitbox.set(42.0, 4.0, -21.0, -16.0);
    entity.collision.platform(true);
    entity.depth.set(1);

    with_state(id, |st| {
        st.start_y = y;
        st.sink = 0.0;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.timer += dt;

        let entity = Entity::new(id);
        let p = entity.position.get();

        // Check if player is standing on car roof
        let has_rider = host::entities_by_type("player").into_iter().any(|pid| {
            if !host::entity_alive(pid) {
                return false;
            }
            let pp = host::Position::new(pid).get();
            let (pw, _, pox, poy) = host::Hitbox::new(pid).get();
            let px = pp.x + pox;
            let py = pp.y + poy;
            (px + pw > p.x - 21.0 && px < p.x + 21.0) && (py >= p.y - 18.0 && py <= p.y - 14.0)
        });

        if has_rider {
            st.sink = (st.sink + dt * 10.0).min(2.0);
        } else {
            st.sink = (st.sink - dt * 10.0).max(0.0);
        }

        entity.position.set_xy(p.x, st.start_y + st.sink);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let p = Entity::new(id).position.get();
        let sway = (st.timer * 0.8).sin() * 0.5;

        // Real car body sprite (scenery/car/body, 47x16).
        draw_image(
            "scenery/car/body",
            p.x - 24.0,
            p.y - 10.0 + sway,
            0.0,
            1.0,
            1.0,
        );
    });
}
