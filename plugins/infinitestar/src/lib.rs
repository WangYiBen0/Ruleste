#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `infiniteStar` entity plugin.
//!
//! The roaming purpose-collectible of chapter 6: a small floating star that
//! vanishes with a pop the instant Madeline touches it and never respawns
//! (`host_collect` marks it consumed for the session). Drawing is procedural
//! (four-point star) so it reads clearly even without the atlas art.

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("infinitestar");
ruleste_plugin_api::ruleste_entity_types!("infiniteStar");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

const HIT: f32 = 16.0;

#[derive(Debug, Default)]
struct StarState {
    timer: f32,
    taken: bool,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<StarState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut StarState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, StarState::default) as *mut StarState;
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
    let _ = spawn.get_bool("shielded", false);
    entity.hitbox.set(HIT, HIT, -HIT * 0.5, -HIT * 0.5);
    entity.depth.set(500);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.timer += dt;
        if st.taken {
            return;
        }
        let p = Entity::new(id).position.get();
        for player_id in host::entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
            let overlap = pp.x + pox < p.x + HIT * 0.5
                && pp.x + pox + pw > p.x - HIT * 0.5
                && pp.y + poy < p.y + HIT * 0.5
                && pp.y + poy + ph > p.y - HIT * 0.5;
            if overlap {
                st.taken = true;
                host::collect(id);
                break;
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        if st.taken {
            return;
        }
        let p = Entity::new(id).position.get();
        let bob = (st.timer * 3.0).sin() * 1.5;
        let y = p.y + bob;
        let pulse = 1.0 + (st.timer * 7.0).sin() * 0.15;
        let c = Color::new(0xf0, 0xe0, 0x50, 0xff);
        // Four-point star: two crossing bars.
        let arm = 5.0 * pulse;
        let cx = p.x;
        host::draw_rect(cx - arm, y - 1.0, arm * 2.0, 2.0, c);
        host::draw_rect(cx - 1.0, y - arm, 2.0, arm * 2.0, c);
        host::draw_rect(
            cx - 1.0,
            y - 1.0,
            2.0,
            2.0,
            Color::new(0xff, 0xff, 0xf0, 0xff),
        );
    });
}
