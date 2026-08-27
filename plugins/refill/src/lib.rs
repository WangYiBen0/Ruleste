#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `refill` entity plugin.
//!
//! Mirrors `Refill.cs`: the green (one dash) / pink (two dash) gems. The player
//! touching one refills their dashes (via an `EV_REFILL` event), the gem
//! briefly vanishes showing an outline, then respawns after 2.5s (or is gone
//! for the session when `oneUse`). Animated with the raw atlas frames since the
//! original builds its sprite directly from the atlas, not a SpriteBank.

use ruleste_plugins_api::host;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::EntityId;

ruleste_plugins_api::ruleste_meta!("refill");
ruleste_plugins_api::ruleste_entity_types!("refill");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const HIT: f32 = 16.0;
const HIT_OX: f32 = -8.0;
const HIT_OY: f32 = -8.0;
const RESPAWN_TIME: f32 = 2.5;
const FLASH_FRAMES: usize = 6;
const FLASH_EVERY: f32 = 2.0;

fn idle_frames(two_dash: bool) -> usize {
    if two_dash { 13 } else { 5 }
}

#[derive(Debug)]
struct RefillState {
    two_dash: bool,
    one_use: bool,
    timer: f32,
    active: bool,
    respawn_timer: f32,
    flash_timer: f32,
    flash_visible: bool,
}

impl Default for RefillState {
    fn default() -> RefillState {
        RefillState {
            two_dash: false,
            one_use: false,
            timer: 0.0,
            active: true,
            respawn_timer: 0.0,
            flash_timer: 0.0,
            flash_visible: false,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<RefillState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut RefillState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, RefillState::default) as *mut RefillState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

fn prefix(two_dash: bool) -> &'static str {
    if two_dash {
        "objects/refillTwo/"
    } else {
        "objects/refill/"
    }
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
    entity.depth.set(-100);
    with_state(id, |st| {
        st.two_dash = spawn.get_bool("twoDash", false);
        st.one_use = spawn.get_bool("oneUse", false);
    });
}

fn frame_id(id: &str) -> String {
    id.to_string()
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.timer += dt;
        if st.respawn_timer > 0.0 {
            st.respawn_timer -= dt;
            if st.respawn_timer <= 0.0 {
                st.active = true;
                st.flash_timer = 0.0;
                st.flash_visible = false;
            }
            return;
        }
        if st.active {
            st.flash_timer += dt;
            st.flash_visible = st.flash_timer % FLASH_EVERY < 0.2;
        }
        let entity = Entity::new(id);
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
                // `Refill.OnPlayer` → `player.UseRefill`: only consume the gem
                // when the player actually needs a refill (dashes below max or
                // stamina under the threshold), otherwise stay put.
                let dashes = host::player_dashes(player_id);
                let stamina = host::player_stamina(player_id);
                let want = if st.two_dash { 2 } else { 1 };
                if dashes >= want && stamina >= 20.0 {
                    continue;
                }
                let mut buf = [0u8; 1];
                buf[0] = u8::from(st.two_dash);
                host::emit(player_id, host::EV_REFILL, &buf);
                st.active = false;
                st.respawn_timer = RESPAWN_TIME;
                if st.one_use {
                    host::collect(id);
                }
                break;
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        if !st.active {
            return;
        }
        let entity = Entity::new(id);
        let p = entity.position.get();
        let bob = (st.timer * 0.6 * 2.0 * std::f32::consts::PI / 2.0).sin() * 2.0;
        let cy = p.y + bob;
        if st.flash_visible {
            let idx = (st.timer * 20.0) as usize % FLASH_FRAMES;
            host::draw_image(
                &frame_id(&format!("{}flash{idx:02}", prefix(st.two_dash))),
                p.x,
                cy,
                0.0,
                1.0,
                1.0,
            );
        } else {
            let idx = (st.timer * 10.0) as usize % idle_frames(st.two_dash);
            host::draw_image(
                &frame_id(&format!("{}idle{idx:02}", prefix(st.two_dash))),
                p.x,
                cy,
                0.0,
                1.0,
                1.0,
            );
        }
    });
}
