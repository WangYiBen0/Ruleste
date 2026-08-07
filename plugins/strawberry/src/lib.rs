//! Strawberry collectible plugin.
//!
//! Mirrors `Strawberry.cs`: a 14x14 centered hitbox, a bobbing idle sprite from
//! the `strawberry` SpriteBank entry, and the touch -> follow -> collect
//! sequence. On touch the berry starts following the player (lagged lerp); the
//! collect timer only advances while the player is on safe ground, so the
//! berry is collected shortly after the player lands. The host permanently
//! consumes it (`host_collect`), so it is not re-created on respawn.

use ruleste_plugin_api::host::{self, entities_by_type};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{spawn_data, Entity, EntityState};
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("strawberry");
ruleste_plugin_api::ruleste_entity_types!("strawberry", "goldenBerry");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

const BERRY_W: f32 = 14.0;
const BERRY_H: f32 = 14.0;
/// Follow delay constant, mirroring `Follower.FollowDelay` (0.3s worth of
/// lerp smoothing).
const FOLLOW_LAG: f32 = 10.0;
const COLLECT_DELAY: f32 = 0.15;

#[derive(Debug)]
struct BerryState {
    start_y: f32,
    wobble: f32,
    following: bool,
    collect_timer: f32,
    golden: bool,
}

impl Default for BerryState {
    fn default() -> BerryState {
        BerryState {
            start_y: 0.0,
            wobble: 0.0,
            following: false,
            collect_timer: 0.0,
            golden: false,
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

#[no_mangle]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let is_golden = spawn.get_str("_entity_type", "strawberry") == "goldenBerry";
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    // Hitbox offset keeps the renderer's bottom-center anchor at the berry's
    // center (its `Position` in the original), so the centered sprite renders
    // exactly where the entity is.
    entity
        .hitbox
        .set(BERRY_W, BERRY_H, -BERRY_W * 0.5, -BERRY_H);
    entity.depth.set(-100);
    if is_golden {
        entity.sprite.set_bank("goldberry");
    }
    with_state(id, |st| {
        st.golden = is_golden;
        st.start_y = y;
    });
    entity.sprite.play("idle");
}

#[no_mangle]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        let entity = Entity::new(id);

        // Bob the sprite ±2px, mirroring Strawberry.Update's sine wobble.
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
                // Follow the player's hitbox center with a lag.
                let target_x = px + pw * 0.5;
                let target_y = py + ph * 0.5;
                let p = entity.position.get();
                let k = (FOLLOW_LAG * dt).min(1.0);
                let nx = p.x + (target_x - p.x) * k;
                let ny = p.y + (target_y - p.y) * k;
                entity.position.set_xy(nx, ny);

                // Collect while the player is on safe ground.
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

#[no_mangle]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let entity = Entity::new(id);
    if !entity.sprite.animation().starts_with("idle") {
        entity.sprite.play("idle");
    }
}
