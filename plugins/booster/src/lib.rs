#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `booster` entity plugin.
//!
//! Mirrors `Booster.cs`: green (`red=false`) and red (`red=true`) pads. The
//! player touching one is boosted: the pad snaps its sprite onto the player,
//! spins while the player dashes out in the boost direction, then pops and
//! respawns after ~1s. Launching happens on the player side (via an `EV_BOOST`
//! event) so it can use the player's own dash physics.

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("booster");
ruleste_plugin_api::ruleste_entity_types!("booster");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

const HIT: f32 = 20.0;
const HIT_OX: f32 = -10.0;
const HIT_OY: f32 = -8.0;
const RESPAWN_TIME: f32 = 1.0;
const SPIN_TIME: f32 = 0.45;
const POP_TIME: f32 = 0.3;

/// The booster's collider is `Circle(10, 0, 2)`: its center sits 2 px below
/// the spawn position.
const CENTER_OY: f32 = 2.0;

/// 0 = idle, 1 = spinning (boosting), 2 = popping, 3 = respawning.
#[derive(Debug, Default)]
struct BoosterState {
    phase: u32,
    timer: f32,
    red: bool,
    /// Free-running animation clock, advanced every update.
    anim: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<BoosterState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut BoosterState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, BoosterState::default) as *mut BoosterState;
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
    entity.hitbox.set(HIT, HIT, HIT_OX, HIT_OY);
    entity.depth.set(-8500);
    with_state(id, |st| {
        st.red = spawn.get_bool("red", false);
    });
}

fn player_overlap(entity: &Entity) -> bool {
    let p = entity.position.get();
    let (w, h, ox, oy) = entity.hitbox.get();
    for player_id in host::entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let overlap = pp.x + pox < p.x + ox + w
            && pp.x + pox + pw > p.x + ox
            && pp.y + poy < p.y + oy + h
            && pp.y + poy + ph > p.y + oy;
        if overlap {
            return true;
        }
    }
    false
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.anim += dt;
        let entity = Entity::new(id);
        match st.phase {
            0 => {
                if player_overlap(&entity) {
                    st.phase = 1;
                    st.timer = SPIN_TIME;
                    let p = entity.position.get();
                    // Payload: booster center (`Circle(10, 0, 2)` → +2 y) then a
                    // red flag, mirroring `Booster.OnPlayer` → `player.Boost` /
                    // `player.RedBoost`. The player uses the target to pull in.
                    let mut buf = [0u8; 9];
                    buf[0..4].copy_from_slice(&p.x.to_le_bytes());
                    buf[4..8].copy_from_slice(&(p.y + CENTER_OY).to_le_bytes());
                    buf[8] = u8::from(st.red);
                    for player_id in host::entities_by_type("player") {
                        if host::entity_alive(player_id) {
                            host::emit(player_id, host::EV_BOOST, &buf);
                            break;
                        }
                    }
                }
            }
            1 => {
                st.timer -= dt;
                if st.timer <= 0.0 {
                    st.phase = 2;
                    st.timer = POP_TIME;
                }
            }
            2 => {
                st.timer -= dt;
                if st.timer <= 0.0 {
                    st.phase = 3;
                    st.timer = RESPAWN_TIME;
                }
            }
            3 => {
                st.timer -= dt;
                if st.timer <= 0.0 {
                    st.phase = 0;
                }
            }
            _ => {}
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let p = entity.position.get();
        let prefix = if st.red {
            "objects/booster/boosterRed"
        } else {
            "objects/booster/booster"
        };
        let idx = match st.phase {
            0 => (st.anim * 10.0) as usize % 5,
            1 => 18 + (st.anim * 17.0) as usize % 8,
            2 => 9 + (st.anim / 0.05) as usize % 9,
            _ => {
                return;
            }
        };
        let frame = format!("{prefix}{idx:02}");
        host::draw_image(&frame, p.x, p.y + 2.0, 0.0, 1.0, 1.0);
    });
}
