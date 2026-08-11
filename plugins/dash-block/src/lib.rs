#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `dashBlock` entity plugin — rewritten to mirror `DashBlock.cs`.
//!
//! The sealed stone slab only gives way to a dash: a `PLAYER_DASH` event arms a
//! short window, and if the player overlaps the slab while that window is live
//! the block breaks (`OnDashCollide`). `permanent` blocks shatter for the
//! session (`host_collect`, kept out of respawns); the rest disappear until the
//! next death respawn (`host_remove`). A `canDash=false` block holds through
//! normal dashes (the original only breaks it via the superdash).

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("dash-block");
ruleste_plugin_api::ruleste_entity_types!("dashBlock");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// How long after a dash starts the block still breaks on contact.
const DASH_WINDOW: f32 = 0.2;

#[derive(Debug, Default)]
struct DashBlockState {
    w: f32,
    h: f32,
    broken: bool,
    can_dash: bool,
    permanent: bool,
    window: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<DashBlockState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut DashBlockState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, DashBlockState::default) as *mut DashBlockState;
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
        st.can_dash = spawn.get_bool("canDash", true);
        st.permanent = spawn.get_bool("permanent", true);
    });
}

fn player_overlaps(id: EntityId) -> bool {
    let p = Entity::new(id).position.get();
    let (w, h, ox, oy) = Entity::new(id).hitbox.get();
    for player_id in host::entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        if pp.x + pox < p.x + ox + w
            && pp.x + pox + pw > p.x + ox
            && pp.y + poy < p.y + oy + h
            && pp.y + poy + ph > p.y + oy
        {
            return true;
        }
    }
    false
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        if st.broken {
            return;
        }
        // `DashListener`-style trigger: every dash arms the window; the block
        // only shatters if the player is actually on it during the window.
        for (_, kind, _) in host::drain_events() {
            if kind == ruleste_plugin_api::plugin::event::PLAYER_DASH {
                st.window = DASH_WINDOW;
                break;
            }
        }
        if st.window > 0.0 {
            st.window -= dt;
            if st.can_dash && player_overlaps(id) {
                st.broken = true;
                if st.permanent {
                    // Shattered for the session (`RemoveAndFlagAsGone`).
                    host::collect(id);
                } else {
                    // Broken this life; returns on the next respawn.
                    host::remove(id);
                }
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        if st.broken {
            return;
        }
        let p = Entity::new(id).position.get();
        host::draw_rect(p.x, p.y, st.w, st.h, Color::new(0x32, 0x3a, 0x48, 0xff));
        host::draw_rect(p.x, p.y, st.w, 2.0, Color::new(0x9a, 0x78, 0x50, 0xff));
    });
}
