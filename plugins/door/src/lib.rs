//! `door` entity plugin.
//!
//! Mirrors `Door.cs`: a depth 8998 door with a `PlayerCollider` hitbox
//! (`Hitbox(12, 22, -6, -23)`) that plays the `open` animation when the
//! player touches it. The SpriteBank `open` -> `close` -> `idle` chain is
//! driven by the host animator's `goto`, so the plugin only triggers `open`
//! and mirrors the sprite based on the player's approach side.

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{spawn_data, Entity, EntityState};
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("door");
ruleste_plugin_api::ruleste_entity_types!("door");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// `Hitbox(12f, 22f, -6f, -23f)`.
const HIT_W: f32 = 12.0;
const HIT_H: f32 = 22.0;
const HIT_OX: f32 = -6.0;
const HIT_OY: f32 = -23.0;

#[derive(Debug)]
struct DoorState {
    /// Whether the door got wedged against solid tiles and can no longer open.
    disabled: bool,
}

impl Default for DoorState {
    fn default() -> DoorState {
        DoorState { disabled: false }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<DoorState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut DoorState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, DoorState::default) as *mut DoorState;
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
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.hitbox.set(HIT_W, HIT_H, HIT_OX, HIT_OY);
    entity.depth.set(8998);
    // `type` is "wood" by default; anything else selects `type + "door"`.
    let kind = spawn.get_str("type", "wood");
    if kind == "wood" {
        entity.sprite.set_bank("door");
    } else {
        entity.sprite.set_bank(&format!("{kind}door"));
    }
    entity.sprite.play("idle");
}

#[no_mangle]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        if !st.disabled && entity.collision.check(0.0, 0.0) {
            st.disabled = true;
        }
        if st.disabled {
            return;
        }
        let p = entity.position.get();
        for player_id in host::entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
            let prx = pp.x + pox;
            let pry = pp.y + poy;
            let overlap = prx < p.x + HIT_OX + HIT_W
                && prx + pw > p.x + HIT_OX
                && pry < p.y + HIT_OY + HIT_H
                && pry + ph > p.y + HIT_OY;
            if overlap {
                open(&entity, pp.x, p.x);
            }
            break;
        }
        let _ = dt;
    });
}

fn open(entity: &Entity, player_x: f32, door_x: f32) {
    let anim = entity.sprite.animation();
    if anim == "idle" {
        entity.sprite.play("open");
        if (player_x - door_x).abs() > 0.001 {
            entity.sprite.flip_x(player_x < door_x);
        }
    }
}
