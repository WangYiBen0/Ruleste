//! Spring entity plugin — bounces the player upward on contact.
//!
//! Validates the cross-plugin architecture: uses `host_entities_by_type` to
//! find players, `host_position_get` to test overlap, and `host_speed_set`
//! to modify the player's velocity.

use ruleste_plugin_api::host::{self, entities_by_type, log};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{spawn_data, Entity, EntityState};
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("spring");
ruleste_plugin_api::ruleste_entity_types!("spring");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

const BOUNCE_SPEED: f32 = -250.0;
const TRIGGER_W: f32 = 12.0;
const TRIGGER_H: f32 = 4.0;

#[derive(Debug, Default)]
struct SpringState {
    cooldown: f32,
}

fn with_state(id: EntityId, f: impl FnOnce(&mut SpringState)) {
    thread_local! {
        static S: std::cell::RefCell<EntityState<SpringState>> =
            std::cell::RefCell::new(EntityState::new());
    }
    S.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, SpringState::default);
        f(st);
    });
}

#[no_mangle]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity = Entity::new(id);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    entity.position.set_xy(x, y);
    entity.hitbox.set(TRIGGER_W, TRIGGER_H, 0.0, 0.0);
    entity.sprite.play("objects/spring/spring");
    with_state(id, |st| *st = SpringState::default());
}

#[no_mangle]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.cooldown -= dt;
        if st.cooldown > 0.0 {
            return;
        }

        let spring = Entity::new(id);
        let sp = spring.position.get();

        for player_id in entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let player_bottom = pp.y + 11.0;
            let player_left = pp.x;
            let player_right = pp.x + 8.0;
            let spring_top = sp.y;
            let spring_left = sp.x - TRIGGER_W * 0.5;
            let spring_right = sp.x + TRIGGER_W * 0.5;

            let overlapping_x = player_right > spring_left && player_left < spring_right;
            let overlapping_y =
                player_bottom >= spring_top && player_bottom <= spring_top + TRIGGER_H + 4.0;

            if overlapping_x && overlapping_y {
                let grounded = host::Collision::new(player_id).is_grounded();
                if grounded {
                    let mut vel = host::Speed::new(player_id).get();
                    vel.y = BOUNCE_SPEED;
                    host::Speed::new(player_id).set(vel);
                    st.cooldown = 0.3;
                    log("spring: bounced player");
                    break;
                }
            }
        }
    });
}

#[no_mangle]
pub extern "C" fn ruleste_entity_draw(_id: EntityId) {}
