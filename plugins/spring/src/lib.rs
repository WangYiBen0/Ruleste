#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! Spring entity plugin — bounces the player on contact, mirroring `Spring.cs`.
//!
//! Three orientations, chosen by entity type:
//! - `spring` (floor): `Player.SuperBounce(top)` — snap to the spring top and
//!   launch straight up at -185 while refilling dash + stamina. Only when the
//!   player is moving down onto it.
//! - `wallSpringLeft` / `wallSpringRight`: `Player.SideBounce(dir, ...)` — a
//!   240 u/s horizontal launch away from the wall.
//!
//! The player is not launched while dashing, and a full-power player is not
//! refilled (guarded on the player side). Bounces use the event bus so the
//! player plugin stays the single owner of player state.

use ruleste_plugins_api::host::{self, entities_by_type};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::EntityId;

ruleste_plugins_api::ruleste_meta!("spring");
ruleste_plugins_api::ruleste_entity_types!("spring", "wallSpringLeft", "wallSpringRight");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

/// StDash (state 9) players never bounce.
const ST_DASH: u32 = 9;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Orientation {
    Floor,
    WallLeft,
    WallRight,
}

#[derive(Debug)]
struct SpringState {
    orientation: Orientation,
    can_use: bool,
    cooldown: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<SpringState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut SpringState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, || unreachable!("state seeded at init")) as *mut SpringState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

fn orientation_from_type(t: &str) -> Orientation {
    match t {
        "wallSpringLeft" => Orientation::WallLeft,
        "wallSpringRight" => Orientation::WallRight,
        _ => Orientation::Floor,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity = Entity::new(id);
    entity
        .position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    entity.depth.set(-8501);
    let orientation = orientation_from_type(&spawn.get_str("_entity_type", "spring"));
    // Original hitboxes, anchored at the entity position.
    let (w, h, ox, oy) = match orientation {
        Orientation::Floor => (16.0, 6.0, -8.0, -6.0),
        Orientation::WallLeft => (6.0, 16.0, 0.0, -8.0),
        Orientation::WallRight => (6.0, 16.0, -6.0, -8.0),
    };
    entity.hitbox.set(w, h, ox, oy);
    entity.sprite.play("objects/spring/spring");
    STATES.with(|s| {
        s.borrow_mut().insert(
            id,
            SpringState {
                orientation,
                can_use: spawn.get_bool("playerCanUse", true),
                cooldown: 0.0,
            },
        );
    });
}

fn push_f32(buf: &mut Vec<u8>, v: f32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Returns the first entity of one of `types` whose hitbox overlaps the spring
/// rect, so the spring can also fire for `Holdable` / `Puffer` / `Seeker`
/// actors (mirroring `Spring.OnHoldable` / `OnPuffer` / `OnSeeker`).
fn overlap_target(types: &[&str], sx: f32, sy: f32, sw: f32, sh: f32) -> Option<EntityId> {
    for t in types {
        for eid in entities_by_type(t) {
            let ep = host::Position::new(eid).get();
            let (ew, eh, eox, eoy) = host::Hitbox::new(eid).get();
            let ex = ep.x + eox;
            let ey = ep.y + eoy;
            if ex < sx + sw && ex + ew > sx && ey < sy + sh && ey + eh > sy {
                return Some(eid);
            }
        }
    }
    None
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.cooldown -= dt;
        if st.cooldown > 0.0 {
            return;
        }

        let spring = Entity::new(id);
        let sp = spring.position.get();
        let (sw, sh, sox, soy) = spring.hitbox.get();
        let sx = sp.x + sox;
        let sy = sp.y + soy;

        for player_id in entities_by_type("player") {
            if !st.can_use || !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
            let overlap = pp.x + pox < sx + sw
                && pp.x + pox + pw > sx
                && pp.y + poy < sy + sh
                && pp.y + poy + ph > sy;
            if !overlap {
                continue;
            }

            let state = host::player_state(player_id);
            let vel = host::Speed::new(player_id).get();
            match st.orientation {
                Orientation::Floor => {
                    // `OnCollide`: only when moving down, never mid-dash.
                    if state != ST_DASH && vel.y >= 0.0 {
                        // `SuperBounce(base.Top)`; top = anchor + collider offset.
                        let from_y = sp.y + soy;
                        let mut buf = Vec::new();
                        push_f32(&mut buf, from_y);
                        host::emit(id, host::EV_SUPER_BOUNCE, &buf);
                        st.cooldown = 0.2;
                        break;
                    }
                }
                Orientation::WallLeft => {
                    if state != ST_DASH {
                        // `SideBounce(1, base.Right, base.CenterY)`: spring on the
                        // left wall, so bounce toward `+x` from its right edge.
                        let from_x = sp.x + sox + sw;
                        let from_y = sp.y + soy + sh * 0.5;
                        let mut buf = Vec::new();
                        buf.push(1);
                        push_f32(&mut buf, from_x);
                        push_f32(&mut buf, from_y);
                        host::emit(id, host::EV_SIDE_BOUNCE, &buf);
                        st.cooldown = 0.2;
                        break;
                    }
                }
                Orientation::WallRight => {
                    if state != ST_DASH {
                        // `SideBounce(-1, base.Left, base.CenterY)`.
                        let from_x = sp.x + sox;
                        let from_y = sp.y + soy + sh * 0.5;
                        let mut buf = Vec::new();
                        buf.push(255);
                        push_f32(&mut buf, from_x);
                        push_f32(&mut buf, from_y);
                        host::emit(id, host::EV_SIDE_BOUNCE, &buf);
                        st.cooldown = 0.2;
                        break;
                    }
                }
            }
        }

        // `OnHoldable` / `OnPuffer` / `OnSeeker`: non-player actors landing on
        // the spring fire it too. Emit a `SPRING_BOUNCE` so the targeted
        // plugin (theo-crystal / key / seeker / puffer) can apply its own
        // launch; the spring just commits its cooldown.
        if let Some(target) =
            overlap_target(&["theo-crystal", "key", "seeker", "puffer"], sx, sy, sw, sh)
        {
            let orientation_byte: u8 = match st.orientation {
                Orientation::Floor => 0,
                Orientation::WallLeft => 1,
                Orientation::WallRight => 2,
            };
            let (from_x, from_y) = match st.orientation {
                Orientation::Floor => (sp.x + sox + sw * 0.5, sp.y + soy),
                Orientation::WallLeft => (sp.x + sox + sw, sp.y + soy + sh * 0.5),
                Orientation::WallRight => (sp.x + sox, sp.y + soy + sh * 0.5),
            };
            let mut buf = Vec::new();
            buf.extend_from_slice(&target.to_le_bytes());
            buf.push(orientation_byte);
            push_f32(&mut buf, from_x);
            push_f32(&mut buf, from_y);
            host::emit(id, host::EV_SPRING_BOUNCE, &buf);
            st.cooldown = 0.2;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(_id: EntityId) {}
