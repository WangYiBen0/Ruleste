#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! Strawberry collectible plugin.
//!
//! Mirrors `Strawberry.cs`: supports normal, golden, moon, and winged variants.
//! - Golden: uses `goldberry` sprite bank.
//! - Moon: uses `moonberry` sprite bank.
//! - Winged: flies upward when player dashes before touching.

use ruleste_plugins_api::host::{self, entities_by_type};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::EntityId;

ruleste_plugins_api::ruleste_meta!("strawberry");
ruleste_plugins_api::ruleste_entity_types!("strawberry", "goldenBerry");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const BERRY_W: f32 = 14.0;
const BERRY_H: f32 = 14.0;
const FOLLOW_LAG: f32 = 10.0;
const COLLECT_DELAY: f32 = 0.15;

#[derive(Debug)]
struct BerryState {
    start_y: f32,
    wobble: f32,
    following: bool,
    collect_timer: f32,
    golden: bool,
    moon: bool,
    winged: bool,
    flying_away: bool,
    fly_timer: f32,
}

impl Default for BerryState {
    fn default() -> BerryState {
        BerryState {
            start_y: 0.0,
            wobble: 0.0,
            following: false,
            collect_timer: 0.0,
            golden: false,
            moon: false,
            winged: false,
            flying_away: false,
            fly_timer: 0.0,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<BerryState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut BerryState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, BerryState::default) as *mut BerryState;
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
    let is_golden = spawn.get_str("_entity_type", "strawberry") == "goldenBerry"
        || spawn.get_bool("golden", false);
    let is_moon = spawn.get_bool("moon", false);
    let is_winged = spawn.get_bool("winged", false);

    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity
        .hitbox
        .set(BERRY_W, BERRY_H, -BERRY_W * 0.5, -BERRY_H);
    entity.depth.set(-100);

    if is_golden {
        entity.sprite.set_bank("goldberry");
    } else if is_moon {
        entity.sprite.set_bank("moonberry");
    } else {
        entity.sprite.set_bank("strawberry");
    }

    with_state(id, |st| {
        st.golden = is_golden;
        st.moon = is_moon;
        st.winged = is_winged;
        st.start_y = y;
        st.flying_away = false;
        st.fly_timer = 0.0;
    });
    entity.sprite.play(if is_winged { "flap" } else { "idle" });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        let entity = Entity::new(id);

        if st.flying_away {
            st.fly_timer += dt;
            let p = entity.position.get();
            entity.position.set_xy(p.x, p.y - 120.0 * dt);
            if st.fly_timer > 3.0 {
                host::remove(id);
            }
            return;
        }

        // Check if player dashed while winged berry is waiting
        if st.winged && !st.following {
            for player_id in entities_by_type("player") {
                if host::entity_alive(player_id) {
                    let states = host::player_state(player_id);
                    // State 2 is Dash in original Celeste
                    if states == 2 {
                        st.flying_away = true;
                        entity.sprite.play("fly");
                    }
                }
            }
        }

        // Bob the sprite ±2px
        if !st.following {
            st.wobble += dt * 4.0;
            let p = entity.position.get();
            entity
                .position
                .set_xy(p.x, st.start_y + st.wobble.sin() * 2.0);
        }

        let mut collecting = false;
        for player_id in entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
            let bx = entity.position.get().x;
            let by = entity.position.get().y;
            let px = pp.x + pox;
            let py = pp.y + poy;

            let touch = !st.following
                && !st.flying_away
                && px < bx + BERRY_W * 0.5
                && px + pw > bx - BERRY_W * 0.5
                && py < by + BERRY_H * 0.5
                && py + ph > by - BERRY_H * 0.5;
            if touch {
                st.following = true;
                st.collect_timer = 0.0;
                host::play_sound(if st.golden {
                    "event:/game/general/strawberry_blue_touch"
                } else {
                    "event:/game/general/strawberry_touch"
                });
            }
            if st.following {
                let target_x = px + pw * 0.5;
                let target_y = py + ph * 0.5;
                let p = entity.position.get();
                let k = (FOLLOW_LAG * dt).min(1.0);
                let nx = p.x + (target_x - p.x) * k;
                let ny = p.y + (target_y - p.y) * k;
                entity.position.set_xy(nx, ny);

                if host::Collision::new(player_id).is_grounded() {
                    st.collect_timer += dt;
                    if st.collect_timer >= COLLECT_DELAY {
                        collecting = true;
                    }
                } else {
                    st.collect_timer = st.collect_timer.min(0.0);
                }
            }
            break;
        }
        if collecting {
            host::play_sound("event:/game/general/strawberry_get");
            host::collect(id);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let entity = Entity::new(id);
    let anim = entity.sprite.animation();
    if anim.is_empty()
        || (!anim.starts_with("idle") && !anim.starts_with("flap") && !anim.starts_with("fly"))
    {
        entity.sprite.play("idle");
    }
}
