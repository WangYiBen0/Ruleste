#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `badelineBoost` entity plugin.
//!
//! The purple storyline launcher (`BadelineBoost`): stepping on the orb
//! launches Madeline along the held direction with full dash momentum. That
//! launch mechanics live on the player side (`EV_BOOST`), exactly like the
//! regular booster — the plugin only watches for contact, fires the event,
//! and draws a pulsing purple orb while it is spent (recharging after a beat).

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("badelineboost");
ruleste_plugin_api::ruleste_entity_types!("badelineBoost");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

const HIT: f32 = 20.0;
const HIT_OX: f32 = -10.0;
const HIT_OY: f32 = -8.0;
const RECHARGE: f32 = 1.2;

#[derive(Debug, Default)]
struct BoostState {
    recharging: f32,
    timer: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<BoostState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut BoostState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, BoostState::default) as *mut BoostState;
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
    entity.hitbox.set(HIT, HIT, HIT_OX, HIT_OY);
    entity.depth.set(-8500);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.timer += dt;
        if st.recharging > 0.0 {
            st.recharging -= dt;
            return;
        }
        let entity = Entity::new(id);
        let p = entity.position.get();
        for player_id in host::entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
            let overlap = pp.x + pox < p.x + HIT_OX + HIT
                && pp.x + pox + pw > p.x + HIT_OX
                && pp.y + poy < p.y + HIT_OY + HIT
                && pp.y + poy + ph > p.y + HIT_OY;
            if overlap {
                host::emit(player_id, host::EV_BOOST, &[]);
                st.recharging = RECHARGE;
                break;
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let p = Entity::new(id).position.get();
        // Purple orb, pulsing while available; dimmed while recharging.
        let pulse = 1.0 + (st.timer * 6.0).sin() * 0.12;
        let on = st.recharging <= 0.0;
        let a = if on { 230 } else { 90 };
        let r = 7.0 * pulse;
        let c = Color::new(0x70, 0x60, 0xe0, a);
        // Approximate the disc with stacked rectangles.
        let mut y = p.y - r;
        while y <= p.y + r {
            let half = (r * r - (y - p.y) * (y - p.y)).sqrt();
            host::draw_rect(p.x - half, y, half * 2.0, 1.0, c);
            y += 2.0;
        }
        host::draw_rect(
            p.x - 2.0,
            p.y - 2.0,
            4.0,
            4.0,
            if on {
                Color::new(0xe0, 0xd0, 0xff, 0xff)
            } else {
                Color::new(0x30, 0x28, 0x50, 0xa0)
            },
        );
    });
}
