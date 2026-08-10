#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `starJumpBlock` entity plugin.
//!
//! The reflection chapter's jump-star blocks: a solid tile slab that, the
//! moment Madeline springs off its underside, becomes a spent star and shoots
//! her upward with a generous reset. `sinks` slabs that already fell away
//! aren't drawn. The block itself is removed via `host_collect` once used.

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId, Vec2};

ruleste_plugin_api::ruleste_meta!("starjumpblock");
ruleste_plugin_api::ruleste_entity_types!("starJumpBlock");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// Upward launch the star bestows, ~the original's powerful star jump.
const STAR_BOOST: f32 = -320.0;

#[derive(Debug, Default)]
struct JumpState {
    w: f32,
    h: f32,
    used: bool,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<JumpState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut JumpState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, JumpState::default) as *mut JumpState;
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
    let w = spawn.get_float("width", 8.0);
    let h = spawn.get_float("height", 8.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.hitbox.set(w, h, 0.0, 0.0);
    entity.depth.set(500);
    with_state(id, |st| {
        st.w = w;
        st.h = h;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, _dt: f32) {
    with_state(id, |st| {
        if st.used {
            return;
        }
        let p = Entity::new(id).position.get();
        for player_id in host::entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
            let overlap = pp.x + pox < p.x + st.w
                && pp.x + pox + pw > p.x
                && pp.y + poy < p.y + st.h
                && pp.y + poy + ph > p.y;
            if !overlap {
                continue;
            }
            let vel = host::Speed::new(player_id).get();
            // The star catches an upward spring: the player is rising when
            // they press off the underside of the slab.
            if vel.y < 0.0 {
                st.used = true;
                host::Speed::new(player_id).set(Vec2::new(vel.x, STAR_BOOST));
                host::collect(id);
                break;
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        if st.used {
            return;
        }
        let p = Entity::new(id).position.get();
        // Star slab: deep purple fill under a starlight cap.
        ruleste_plugin_api::host::draw_rect(
            p.x,
            p.y,
            st.w,
            st.h,
            Color::new(0x60, 0x38, 0xa8, 0xff),
        );
        ruleste_plugin_api::host::draw_rect(
            p.x,
            p.y,
            st.w,
            2.0,
            Color::new(0xd0, 0xb0, 0xf8, 0xff),
        );
    });
}
